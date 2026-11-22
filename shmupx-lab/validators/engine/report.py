"""Contrato versionado para consumidores nativos; o JSON legado permanece disponível."""


def client_report(game_id, version, result, gameplay):
    # Continues ainda usam heurísticas sem corpus negativo validado.
    # Game over é o fim normal de uma partida de 1 ficha (morreu na última vida): o placar final
    # do HUD vale e não pede revisão.
    # Partida com 2 jogadores não vale para o ranking de 1 jogador sem alguém olhar.
    needs_review = (result["confidence"] != "high" or result["hi"] is None
                    or result["continues"] > 0 or result.get("players", 1) > 1)
    return {
        "schema_version": 1,
        "game_id": game_id,
        "validator_version": version,
        "confidence": result["confidence"],
        "needs_review": needs_review,
        "gameplay": gameplay,
    }
