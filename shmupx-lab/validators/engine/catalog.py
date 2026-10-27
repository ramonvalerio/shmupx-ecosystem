"""Catálogo de scripts: cada pasta games/<jogo>/<versão>/ com um script.json é um script.

IDs e versões nunca viram caminhos ou imports livres: são conferidos por expressão regular, e o
analisador é escolhido por nome dentro de ANALYZERS.
"""
import json
import re
from dataclasses import dataclass
from importlib import import_module
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GAMES_DIR = ROOT / "games"
GAME_ID = re.compile(r"^[a-z0-9][a-z0-9_-]{0,63}$")
VERSION = re.compile(r"^(0|[1-9]\d{0,3})\.(0|[1-9]\d{0,3})\.(0|[1-9]\d{0,3})$")
# O script.json escolhe o analisador por nome, nunca por caminho de módulo.
ANALYZERS = {"hud_score": "engine.analyzers.hud_score"}
STATUSES = ("draft", "approved")


@dataclass(frozen=True)
class Script:
    game_id: str
    name: str
    version: str
    status: str
    analyzer: str
    path: Path

    @property
    def version_key(self):
        return tuple(int(part) for part in self.version.split("."))

    def create(self, video_path, *, debug_dir=None, quiet=False):
        # Import tardio: listar scripts e consultar ajuda não requer OpenCV.
        module = import_module(ANALYZERS[self.analyzer])
        return module.create_analyzer(self.path, video_path, debug_dir=debug_dir, quiet=quiet)

    def to_json(self):
        return {"id": self.game_id, "name": self.name, "version": self.version, "status": self.status}


def _load(game_dir, version_dir):
    manifest_path = version_dir / "script.json"
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise ValueError(f"{manifest_path}: script.json ilegível ({exc})") from exc
    if manifest.get("game") != game_dir.name or manifest.get("version") != version_dir.name:
        raise ValueError(f"{manifest_path}: 'game' e 'version' precisam ser iguais aos nomes das pastas")
    if manifest.get("analyzer") not in ANALYZERS:
        raise ValueError(f"{manifest_path}: analisador desconhecido {manifest.get('analyzer')!r}")
    if manifest.get("status") not in STATUSES:
        raise ValueError(f"{manifest_path}: status deve ser um de {', '.join(STATUSES)}")
    name = str(manifest.get("name") or game_dir.name)
    return Script(game_dir.name, name, version_dir.name, manifest["status"], manifest["analyzer"], version_dir)


def scripts(games_dir=GAMES_DIR):
    """Todos os scripts, por jogo e versão crescente."""
    found = []
    games_dir = Path(games_dir)
    if not games_dir.is_dir():
        return found
    for game_dir in games_dir.iterdir():
        if not game_dir.is_dir() or not GAME_ID.match(game_dir.name):
            continue
        for version_dir in game_dir.iterdir():
            if version_dir.is_dir() and VERSION.match(version_dir.name) and (version_dir / "script.json").is_file():
                found.append(_load(game_dir, version_dir))
    found.sort(key=lambda s: (s.game_id, s.version_key))
    return found


def get_script(game_id, version=None, games_dir=GAMES_DIR):
    """Script do jogo na versão pedida; sem versão, a mais nova aprovada (ou a mais nova, se nenhuma for)."""
    if not isinstance(game_id, str) or not GAME_ID.match(game_id):
        raise ValueError(f"Jogo não suportado: {game_id}")
    if version is not None and not VERSION.match(version):
        raise ValueError(f"Versão inválida: {version}")
    all_scripts = scripts(games_dir)
    candidates = [s for s in all_scripts if s.game_id == game_id]
    if not candidates:
        supported = ", ".join(sorted({s.game_id for s in all_scripts}))
        raise ValueError(f"Jogo não suportado: {game_id}. Disponíveis: {supported}")
    if version is None:
        approved = [s for s in candidates if s.status == "approved"]
        return (approved or candidates)[-1]
    for script in candidates:
        if script.version == version:
            return script
    versions = ", ".join(s.version for s in candidates)
    raise ValueError(f"Versão {version} de {game_id} não encontrada. Disponíveis: {versions}")
