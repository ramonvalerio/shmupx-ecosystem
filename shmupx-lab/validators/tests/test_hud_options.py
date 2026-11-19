"""Opções do HUD usadas por jogos além do Earthion: placar alinhado à direita e vidas por ícone."""
import unittest

import numpy as np

from engine.hud import Hud

RNG = np.random.default_rng(7)
# "Dígitos" sintéticos bem diferentes entre si: padrões claros (>180) sobre fundo escuro.
DIGITS = {str(d): np.where(RNG.random((16, 12)) > 0.5, 230.0, 20.0).astype(np.float32) for d in range(10)}
ICON = np.where(RNG.random((14, 12)) > 0.5, 200.0, 10.0).astype(np.float32)


class FakeConfig(dict):
    def __init__(self, **cfg):
        super().__init__(cfg)
        self.digit_sets = {"arcade": DIGITS}
        self.icons = {"lives": [(0, None, ICON)]}


def config(**score):
    base = dict(dx=38, dy=1, count=8, pitch=13.0, pad=3, align="right", white_level=180, blank_max_white=0.06)
    base.update(score)
    return FakeConfig(score=base, digit_size=[12, 16], digit_min_score=0.72, digit_min_margin=0.02,
                      lives=dict(mode="icons", dx=-2, dy=18, pitch=13.0, pad=3, max=9, min_score=0.7,
                                 doubt_margin=0.05))


def hud_for(cfg):
    hud = Hud(cfg)
    hud.label_pos = (40.0, 5.0)
    hud.offsets = {"score": (38.0, 1.0), "lives": (-2.0, 18.0)}
    return hud


def frame_with(score="", icons=0):
    g = np.full((60, 200), 20.0, dtype=np.float32)
    first_cell = 8 - len(score)
    for i, d in enumerate(score):
        x = 78 + 13 * (first_cell + i)
        g[6:22, x:x + 12] = DIGITS[d]
    for k in range(icons):
        x = 38 + 13 * k
        g[23:37, x:x + 12] = ICON
    return g


class RightAlignedScoreTests(unittest.TestCase):
    def test_reads_numbers_of_any_length_without_leading_zeros(self):
        hud = hud_for(config())
        for score in ("54000", "1131600", "12345678", "7"):
            s, ok, _, _ = hud.read_score(frame_with(score))
            self.assertEqual((s, ok), (score, True))

    def test_empty_hud_is_not_a_reading(self):
        _, ok, _, _ = hud_for(config()).read_score(frame_with(""))
        self.assertFalse(ok)

    def test_leading_zero_or_wrong_multiple_is_rejected(self):
        self.assertFalse(hud_for(config()).read_score(frame_with("0540"))[1])
        self.assertTrue(hud_for(config(multiple_of=100)).read_score(frame_with("247600"))[1])
        self.assertFalse(hud_for(config(multiple_of=100)).read_score(frame_with("247602"))[1])


class LifeIconTests(unittest.TestCase):
    def test_counts_icons_side_by_side(self):
        hud = hud_for(config())
        for icons in (0, 1, 3, 6):
            count, ok, _ = hud.read_small(frame_with(icons=icons), "lives")
            self.assertEqual((count, ok), (icons, True))

    def test_icon_out_of_step_does_not_count(self):
        g = frame_with(icons=2)
        x = 38 + 13 * 2 + 3  # terceiro "ícone" 3 px fora do passo (ex.: tiro azul parecido)
        g[23:37, x:x + 12] = ICON
        count, _, _ = hud_for(config()).read_small(g, "lives")
        self.assertEqual(count, 2)


if __name__ == "__main__":
    unittest.main()
