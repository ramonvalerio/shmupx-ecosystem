"""Contrato versionado para consumidores nativos; o JSON legado permanece disponível."""


def client_report(game_id, version, result, gameplay):
    # Continues/game over ainda usam heurísticas sem corpus negativo validado.
    needs_review = (result["confidence"] != "high" or result["hi"] is None
                    or result["continues"] > 0 or result["game_over"])
    return {
        "schema_version": 1,
        "game_id": game_id,
        "validator_version": version,
        "confidence": result["confidence"],
        "needs_review": needs_review,
        "gameplay": gameplay,
    }
