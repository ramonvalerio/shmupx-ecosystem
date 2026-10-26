"""Registro explícito de scripts confiáveis; IDs nunca viram caminhos/imports livres."""
from dataclasses import dataclass
from importlib import import_module
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


@dataclass(frozen=True)
class ValidatorSpec:
    game_id: str
    name: str
    version: str
    module: str

    def create(self, video_path, *, debug_dir=None, quiet=False):
        # Import tardio: listar jogos e consultar ajuda não requer OpenCV.
        module = import_module(self.module)
        return module.create_analyzer(
            ROOT, video_path, debug_dir=debug_dir, quiet=quiet,
        )


VALIDATORS = (
    ValidatorSpec("earthion", "Earthion", "1.0.0", "shmupx.validators.earthion"),
)


def get_validator(game_id):
    for spec in VALIDATORS:
        if spec.game_id == game_id:
            return spec
    supported = ", ".join(spec.game_id for spec in VALIDATORS)
    raise ValueError(f"Jogo não suportado: {game_id}. Disponíveis: {supported}")
