# Spec 001 — Domínio de Análise e Perfis (ciclo de vida do Lab ao Client)

- **Status:** Rascunho
- **Relacionado:** ADR 002

## Entidades

- **AnalysisProfile**: `{ gameId, version, variant: "lcd" | "crt", regions, modelRef, sha256, createdAt }`
- **Region**: retângulo normalizado `{ x, y, width, height }` em 0..1 do frame de referência.
- **ProfileRelease**: perfil publicado (imutável) com canal (`draft` | `stable`).

## Ciclo de vida

1. **Importar dataset (Lab):** vídeos e screenshots locais, indexados por jogo.
2. **Mapear (Lab):** no canvas, o admin desenha regiões de placar, vidas, continues e game over sobre um frame de referência.
3. **Treinar/validar (Lab):** treino local de pesos leves; exporta ONNX; mede acurácia contra um conjunto de verificação.
4. **Versionar:** cada alteração gera nova versão semver; criar variantes `lcd` e `crt` a partir de um perfil base, ajustando regiões e/ou pesos.
5. **Publicar (Lab → Supabase Storage):** upload de `profile.json` + `model.onnx` em `profiles/{gameId}/{version}/{variant}/`; registro na tabela `profile_releases` com o SHA-256 do conteúdo.
6. **Sincronizar (Client):** em segundo plano, consulta releases `stable` mais novos que os locais; baixa, valida o SHA-256 e só então troca o ponteiro ativo (atômico). Falha = mantém a versão anterior.
7. **Selecionar variante (Client):** escolhe `lcd` ou `crt` conforme configuração/detecção do jogador; fallback para `lcd`.
8. **Analisar:** ao fim da run, amostra frames do vídeo, aplica regiões + modelo e produz `AnalysisResult { score, continues, livesLost, gameOver, confidence }`.

## Regras

- Perfis publicados são **imutáveis**; correção = nova versão.
- Todo resultado registra a `profileVersion` e a `variant` usadas, para permitir reprocessamento.
- Confiança abaixo de um limiar marca a run como `needs_review` em vez de rejeitá-la.

## Fora de escopo (por ora)

Assinatura criptográfica de releases (avaliar após o primeiro ciclo ponta a ponta).
