import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

from engine.catalog import ROOT, get_script
from engine.cli import main


class CliTests(unittest.TestCase):
    def test_help_uses_utf8_in_windows_pipes(self):
        result = subprocess.run([sys.executable, str(ROOT / "analyze.py"), "--help"],
                                capture_output=True, encoding="utf-8", check=True)
        self.assertIn("cotação US$→R$", result.stdout)

    def test_listing_does_not_load_computer_vision(self):
        script = ("from engine.cli import main; main(['--list-games']); import sys; "
                  "assert 'cv2' not in sys.modules; assert 'engine.grok' not in sys.modules")
        result = subprocess.run([sys.executable, "-c", script], cwd=ROOT,
                                capture_output=True, text=True, check=True)
        listed = json.loads(result.stdout)
        self.assertIn({"id": "earthion", "name": "Earthion", "version": "1.0.0", "status": "approved", "missing": []}, listed)

    def test_entrypoint_works_outside_project_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run([sys.executable, str(ROOT / "analyze.py"), "--list-games"],
                                    cwd=directory, capture_output=True, text=True, check=True)
        self.assertTrue(any(s["id"] == "earthion" for s in json.loads(result.stdout)))

    def test_games_dir_points_to_scripts_outside_the_engine(self):
        # O .exe do Client recebe os scripts como dados, numa pasta separada do motor.
        from tests.test_catalog import write_script
        with tempfile.TemporaryDirectory() as games:
            write_script(games, "ketsui", "2.0.0")
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                main(["--list-games", "--games-dir", games])
        self.assertEqual([(s["id"], s["version"]) for s in json.loads(out.getvalue())], [("ketsui", "2.0.0")])

    def test_earthion_resources_load_from_the_version_folder(self):
        from engine.hud import GameConfig
        config = GameConfig(get_script("earthion", "1.0.0").path)
        self.assertTrue(config.variants)
        self.assertEqual(len(config.digit_sets), 3)
        for digits in config.digit_sets.values():
            self.assertEqual(set(digits), set("0123456789"))

    def test_progress_printer_reports_each_new_percent_up_to_the_limit(self):
        from engine.cli import progress_printer
        stream = io.StringIO()
        report = progress_printer(stream, limit=90)
        for fraction in (0.0, 0.004, 0.5, 0.5, 0.3, 1.0, 1.7):
            report(fraction)
        self.assertEqual(stream.getvalue().splitlines(),
                         ["SHMUPX_PROGRESS 0", "SHMUPX_PROGRESS 45", "SHMUPX_PROGRESS 90"])

    def test_external_audit_requires_explicit_opt_in(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as error:
            main(["video.mp4", "--game", "earthion", "--player", "P", "--grok-always"])
        self.assertEqual(error.exception.code, 2)

    def test_incomplete_script_stops_before_reading_the_video(self):
        spec = Mock(version="0.1.0")
        spec.missing_resources.return_value = ["templates/labels/hud_score.png"]
        with tempfile.TemporaryDirectory() as directory:
            video = Path(directory) / "video.mp4"
            video.touch()
            with patch("engine.cli.get_script", return_value=spec), self.assertRaises(SystemExit) as error:
                main([str(video), "--game", "ibara", "--player", "P"])
        self.assertIn("hud_score.png", str(error.exception.code))
        spec.create.assert_not_called()

    def test_earthion_has_every_resource(self):
        self.assertEqual(get_script("earthion", "1.0.0").missing_resources(), [])

    def test_unknown_version_is_a_usage_error(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as error:
            main(["video.mp4", "--game", "earthion", "--version", "9.9.9", "--player", "P"])
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
        spec = Mock(version="1.0.0")
        spec.missing_resources.return_value = []
        spec.create.return_value = analyzer
        stdout, stderr = io.StringIO(), io.StringIO()
        with tempfile.TemporaryDirectory() as directory:
            video = Path(directory) / "video.mp4"
            video.touch()
            output = Path(directory) / "result.json"
            with patch("engine.cli.get_script", return_value=spec), \
                 contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                main([str(video), "--game", "earthion", "--player", "Jogador", "--out", str(output)])
            payload = json.loads(stdout.getvalue())
            self.assertEqual(payload, json.loads(output.read_text(encoding="utf-8")))
        self.assertEqual(payload["earthion"]["hi-score"], 1234)
        self.assertEqual(payload["earthion"]["total_tokens"], None)
        self.assertIn("progresso", stderr.getvalue())
        analyzer.v.cap.release.assert_called_once()

    def test_client_report_carries_the_script_version(self):
        analyzer = Mock()
        analyzer.v.duration = 60
        analyzer.v.frames_read = 30
        analyzer.run.return_value = dict(hi=10, confidence="high", continues=0, lives_lost=0,
                                         game_over=False, attempts=0, observation=None, deaths=[])
        spec = Mock(version="1.2.0")
        spec.missing_resources.return_value = []
        spec.create.return_value = analyzer
        stdout = io.StringIO()
        with tempfile.TemporaryDirectory() as directory:
            video = Path(directory) / "video.mp4"
            video.touch()
            with patch("engine.cli.get_script", return_value=spec), contextlib.redirect_stdout(stdout):
                main([str(video), "--game", "earthion", "--version", "1.2.0", "--player", "P",
                      "--format", "client-v1", "--quiet"])
        self.assertEqual(json.loads(stdout.getvalue())["validator_version"], "1.2.0")

    def test_failure_releases_video_without_success_json(self):
        analyzer = Mock()
        analyzer.run.side_effect = SystemExit("HUD não encontrado")
        spec = Mock(version="1.0.0")
        spec.missing_resources.return_value = []
        spec.create.return_value = analyzer
        output = io.StringIO()
        with tempfile.TemporaryDirectory() as directory:
            video = Path(directory) / "video.mp4"
            video.touch()
            with patch("engine.cli.get_script", return_value=spec), \
                 contextlib.redirect_stdout(output), self.assertRaises(SystemExit):
                main([str(video), "--game", "earthion", "--player", "P"])
        self.assertEqual(output.getvalue(), "")
        analyzer.v.cap.release.assert_called_once()


if __name__ == "__main__":
    unittest.main()
