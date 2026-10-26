# ShmupX Ecosystem

Monorepositório do ShmupX. Cada aplicação mantém suas dependências e comandos de execução; decisões compartilhadas ficam em `docs/`.

| Diretório | Responsabilidade |
| --- | --- |
| `shmupx-client/` | Aplicativo do jogador: interface React, sessão, captura e integração nativa Tauri/Rust. |
| `shmupx_offline/` | Validação local de vídeos, com um script específico por jogo. Earthion é o primeiro. |
| `shmupx-lab/` | Ferramentas administrativas de mapeamento e preparação de recursos de análise. |
| `shmupx-backend/` | API Rust, persistência, regras de submissão e ranking. |
| `shmupx-frontend/` | Portal web público. |
| `shmupx-infra/` | Infraestrutura e configuração de ambientes. |
| `docs/adr/` | Decisões de arquitetura e sua evolução. |
| `docs/specs/` | Contratos e regras entre componentes. |

## Validação offline por jogo

O cliente grava o vídeo; o validador do jogo extrai os dados da gameplay localmente. O backend continua responsável pela decisão de aceitar uma submissão no ranking. Extração de placar não é prova completa de integridade da partida.

O módulo Python já pode ser executado separadamente. O cliente já chama o adaptador Python ao encerrar a captura, sem fallback de pontuação. O fluxo completo ainda depende do catálogo real (UUID do backend associado ao ID do script), autenticação, Python instalado e validação com vídeos reais. O ONNX experimental permanece no código, fora do caminho de submissão. Consulte [a decisão vigente](docs/adr/004-validadores-offline-por-jogo.md), [o contrato](docs/specs/003-validadores-offline.md) e [o guia do validador](shmupx_offline/README.md).

```powershell
cd shmupx_offline
python analyze.py --list-games
python analyze.py "D:\videos\earthion.mp4" --game earthion --player "Jogador" --out resultado.json
python -m unittest discover -s tests -v
```

Instale primeiro as dependências do módulo em ambiente virtual conforme seu README. O fluxo padrão não utiliza rede.

## Organização do desenvolvimento

- Regras específicas de jogo ficam no seu validador, fora dos componentes de interface.
- Utilitários compartilhados só devem conter comportamento realmente comum.
- Não colocar vídeos, ambientes virtuais, resultados ou credenciais no repositório. Use `shmupx_offline/output/` para resultados locais.
- Testes ficam junto ao projeto responsável. Não há um instalador ou comando de build único para todas as stacks.
- A migração visual do desktop está registrada em [LAYOUT-MIGRATION.md](shmupx-client/LAYOUT-MIGRATION.md).


## Banco da plataforma: Firebase

O destino definido é Firebase Authentication + Firestore Standard, cobrindo perfis, jogos, eventos, gameplays, análises, fichas e rankings. A configuração local e seus testes ficam em [shmupx-infra/firebase](shmupx-infra/firebase/README.md). O banco já foi criado no console e suas regras e índices foram publicados após nove testes de acesso. A integração do aplicativo e a migração do backend PostgreSQL/Supabase ainda estão pendentes. Os vídeos continuam locais no estágio inicial.
