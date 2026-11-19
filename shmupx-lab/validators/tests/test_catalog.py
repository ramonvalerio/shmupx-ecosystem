import json
from pathlib import Path
import tempfile
import unittest

from engine.catalog import get_script, scripts


def write_script(games, game, version, status="approved", **extra):
    folder = Path(games) / game / version
    folder.mkdir(parents=True)
    manifest = dict(game=game, name=game.title(), version=version, status=status, analyzer="hud_score")
    manifest.update(extra)
    (folder / "script.json").write_text(json.dumps(manifest), encoding="utf-8")
    return folder


class CatalogTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.games = self.directory.name

    def tearDown(self):
        self.directory.cleanup()

    def test_each_version_folder_is_a_script_in_numeric_order(self):
        write_script(self.games, "ketsui", "1.10.0")
        write_script(self.games, "ketsui", "1.2.0")
        write_script(self.games, "earthion", "1.0.0")
        listed = [(s.game_id, s.version) for s in scripts(self.games)]
        self.assertEqual(listed, [("earthion", "1.0.0"), ("ketsui", "1.2.0"), ("ketsui", "1.10.0")])

    def test_default_is_the_newest_approved_version(self):
        write_script(self.games, "ketsui", "1.0.0")
        write_script(self.games, "ketsui", "1.1.0", status="draft")
        self.assertEqual(get_script("ketsui", games_dir=self.games).version, "1.0.0")
        self.assertEqual(get_script("ketsui", "1.1.0", games_dir=self.games).status, "draft")

    def test_game_with_only_drafts_uses_the_newest_draft(self):
        write_script(self.games, "ibara", "0.1.0", status="draft")
        write_script(self.games, "ibara", "0.2.0", status="draft")
        self.assertEqual(get_script("ibara", games_dir=self.games).version, "0.2.0")

    def test_ids_and_versions_never_become_paths(self):
        write_script(self.games, "earthion", "1.0.0")
        for game in ("../earthion", "Earthion", "unknown", "earthion.py", ""):
            with self.assertRaises(ValueError):
                get_script(game, games_dir=self.games)
        for version in ("../1.0.0", "1.0", "01.0.0", "9.9.9"):
            with self.assertRaises(ValueError):
                get_script("earthion", version, games_dir=self.games)

    def test_manifest_must_match_its_folder_and_name_a_known_analyzer(self):
        write_script(self.games, "ketsui", "1.0.0", version_override=None)
        bad = write_script(self.games, "ibara", "1.0.0", analyzer="os.path")
        with self.assertRaises(ValueError):
            scripts(self.games)
        (bad / "script.json").write_text(json.dumps(dict(game="outro", version="1.0.0", status="approved",
                                                         analyzer="hud_score")), encoding="utf-8")
        with self.assertRaises(ValueError):
            scripts(self.games)

    def test_missing_resources_lists_labels_and_digits(self):
        folder = write_script(self.games, "ibara", "0.1.0", status="draft")
        config = {"hud_variants": [{"label_template": "labels/score.png", "alt_label_templates": ["labels/boss.png"]}],
                  "results": {"label_templates": ["labels/total.png"]}}
        (folder / "config.json").write_text(json.dumps(config), encoding="utf-8")
        script = get_script("ibara", games_dir=self.games)
        self.assertEqual(script.missing_resources(), ["templates/labels/score.png", "templates/labels/boss.png",
                                                      "templates/labels/total.png",
                                                      "templates/digits/<conjunto>/0.png … 9.png"])
        for name in ("score", "boss", "total"):
            (folder / "templates" / "labels").mkdir(parents=True, exist_ok=True)
            (folder / "templates" / "labels" / f"{name}.png").write_bytes(b"png")
        digits = folder / "templates" / "digits" / "pc"
        digits.mkdir(parents=True)
        for n in range(9):
            (digits / f"{n}.png").write_bytes(b"png")
        self.assertEqual(len(script.missing_resources()), 1, "conjunto sem o 9 não serve")
        (digits / "9.png").write_bytes(b"png")
        self.assertEqual(script.missing_resources(), [])

    def test_folders_without_manifest_or_with_odd_names_are_ignored(self):
        write_script(self.games, "earthion", "1.0.0")
        (Path(self.games) / "earthion" / "rascunho").mkdir()
        (Path(self.games) / "earthion" / "2.0.0").mkdir()
        (Path(self.games) / "Nao_Valido").mkdir()
        self.assertEqual([s.version for s in scripts(self.games)], ["1.0.0"])


if __name__ == "__main__":
    unittest.main()
