# Migração do layout WinUI

Referência: `../../shmupx-client/src/ShmupX.Client.App`.

O shell React/Tauri reproduz a estrutura de `MainWindow.xaml`: trilho de navegação de 40 px com abas verticais, barra superior de 62 px com logo e estado, coleção de jogos com painel lateral e rodapé de estado. A paleta vem de `Themes/Palette.xaml`: superfícies #1F1F1F, #262626 e #313131, seleção #57FCFF, títulos #49C9C9 e estado positivo #3FD98B. A tipografia utiliza Segoe UI, disponível no Windows, sem carregar fontes externas.

`src/components/ClientShell.tsx` contém a navegação, busca local e cartões da coleção. `src/index.css` contém os estilos e adaptações para janelas menores. O logo e a capa curada de DoDonPachi foram copiados do cliente original para `public/img`; os demais jogos usam cartões tipográficos de fallback.

As salas, mensagens, fichas e rankings continuam usando as fontes de demonstração já existentes no destino. Os comandos Tauri de inicialização do jogo e parada de gravação, eventos de início/expiração e estado de autenticação foram preservados. Coleção e Salas oferecem duas disposições para os mesmos jogos, sem afirmar que a biblioteca Steam foi sincronizada.

## Segunda etapa

A navegação agora inclui Início, Coleção, Eventos, Ranking, Salas, Configurações e Ajuda. As telas ficam em `src/components/ClientPages.tsx`.

- Início: cinco painéis seguindo a organização de `HomeDashboardView.xaml`, com reordenação por botões e alternância entre compacto/amplo durante a sessão. O desempenho usa um estado vazio enquanto não houver histórico; comunidade e ranking usam as fontes de demonstração existentes. Atalhos abrem a coleção, o jogo, o ranking e eventos.
- Eventos: as três artes, títulos e períodos de `EventCatalog.cs` foram migrados. Busca, filtro de status e detalhes funcionam localmente. Status é calculado pelas datas e atualizado a cada 30 segundos; datas são apresentadas em São Paulo (UTC−3). Não há inscrição online nesta etapa.
- Ranking: seleção de jogo, busca por jogador e painel de detalhes, usando `gerarRankingSalaMock` e preservando a classificação original durante a busca.
- Configurações: menu de grupos do original, tema padrão/escuro e coleção compacta com persistência versionada no armazenamento local. Demais grupos explicam os parâmetros atuais e os recursos ainda indisponíveis, sem oferecer controles que o backend não suporta.
- Ajuda: fluxo da run e diagnóstico com consulta real ao comando Tauri `app_version`, além de estado específico para a prévia no navegador.

O painel de sala permanece montado ao mudar de seção, preservando chat, seleção e gravação. O aviso de expiração fica fora das páginas e permanece visível em qualquer seção. Na prévia no navegador, os efeitos exclusivos do Tauri são ignorados.

## Terceira etapa

- Big Picture: apresentação maior do mesmo shell, navegação horizontal, foco direcional por setas, ativação por Enter e alternância por botão/Alt+Enter; Esc retorna à apresentação em janela. Seleção, filtros, mensagens e a instância de `RoomView` são compartilhados, sem iniciar jogos ao apenas selecionar um cartão. No desktop, o modo usa `setFullscreen` do Tauri, com a permissão específica `core:window:allow-set-fullscreen`; a prévia no navegador altera apenas o layout.
- Perfil: menu no avatar, tela de identidade local, edição de nome com limite de 32 caracteres e persistência versionada no armazenamento local. Não representa autenticação ou edição da conta online.
- Assinatura: composição de `SubscriptionView.cs`, com Cartão/Pix, campos de exemplo somente para leitura, resumo e revisão demonstrativa. Não cria cobrança ou assinatura, nem coleta dados de pagamento.
- Submissões: lista, busca, filtro e painel de detalhes para runs observadas nesta sessão. Uma run aparece apenas após sucesso em `iniciar_corrida_safe`; `Enviada` é aplicado após o retorno bem-sucedido do comando de encerramento/envio. O evento de expiração indica somente `Tempo encerrado`, pois não confirma o envio. Não são gerados placares, resultados de verificação ou histórico fictício. O histórico não persiste ao fechar/recarregar o cliente.

Verificação da terceira etapa: build TypeScript/Vite aprovado; prévia conferida com entrada/saída de Big Picture, setas/Enter, seleção de jogo preservada no retorno à janela, estado vazio de submissões, nome de perfil persistido após reload, Cartão/Pix e revisão demonstrativa. A prévia nova não apresentou erros no console. Tela cheia nativa, gravação e preenchimento das submissões a partir de runs reais ainda precisam de validação no Tauri com Steam, FFmpeg e backend disponíveis.

## Quarta etapa

- Navegação por gamepad no Big Picture: Gamepad API, mapeamento `standard`, direcional ou analógico esquerdo, A para confirmar e B para fechar o diálogo/menu ou sair do modo. Seletores usam esquerda/direita para alterar a opção; campos de texto continuam usando teclado. O cliente só navega enquanto tem foco e está visível. Ao entrar, reconectar ou recuperar foco, exige estado neutro antes de aceitar comandos. Somente movimentos repetem (400 ms iniciais, 160 ms entre repetições); confirmação e voltar não repetem enquanto mantidos. A navegação não usa Select/Back do controle, preservando o botão nativo de push-to-talk. O rodapé mostra se há um controle compatível detectado.
- Dashboard: ordem, largura compacta/ampla e altura entre 240 e 800 px persistem em `shmupx.client.dashboard.v1`. Dados inválidos retornam ao layout original. Alças permitem mover e redimensionar com mouse/toque; botões oferecem alternativas por teclado. Esc cancela uma operação em andamento. A opção Restaurar layout recupera a organização original. Falha de armazenamento mantém a edição na sessão e exibe aviso.
- Diálogos de ranking e expiração: foco inicial, Tab/Shift+Tab limitados ao diálogo visível, fechamento por Esc e restauração do foco. A navegação direcional por controle respeita o diálogo aberto; B usa o botão de fechamento, sem sair do Big Picture.

Validação desta etapa: `npm run build` e `npm run test:ui` (cinco testes cobrindo confirmação mantida, retomada de foco, repetição direcional, zona neutra do analógico, validação e reordenação dos painéis). Na prévia, confirmados movimento por alça, redimensionamento pelo canto e por botão, persistência da ordem/altura após recarregar, restauração e Tab/Esc no ranking com retorno ao botão de origem, mantendo Big Picture ativo. Não havia um controle físico disponível na verificação; conexão e comportamento real do gamepad no WebView/Tauri ainda precisam ser conferidos no dispositivo.

## Quinta etapa

- Coleção: componente dedicado `CollectionPane.tsx`, com busca sem distinção de acentos, todos/favoritos/com runs da sessão, ordenação por nome, favoritos ou gravação recente, direção alfabética e grade/lista. Favoritos, modo de exibição e ordenação persistem em `shmupx.client.collection.v1`; busca e filtro ficam na sessão. Os favoritos são botões independentes dos cartões, sem botões aninhados. No Big Picture a apresentação usa grade, preservando a preferência de lista para o retorno ao modo em janela.
- Seleção e filtros: esconder o jogo pelo filtro mantém a sala e os controles de gravação; um aviso identifica o jogo selecionado e permite limpar os filtros. Há estados vazios específicos para favoritos, runs e busca sem resultados.
- Detalhes: painel alterna entre Sala e Detalhes, com capa disponível, plataforma, atividade de demonstração, estado de gravação e instruções. Chat continua montado quando oculto pela aba Detalhes. Não se atribui instalação ou suporte de análise sem dados do serviço.

Validação: build aprovado e oito testes de UI passando, incluindo busca sem acentos, favoritos, ordenação por timestamps reais da sessão e preferências inválidas. Na prévia, conferidos favorito persistente após reload, modo de lista persistente, filtro vazio de runs mantendo o jogo selecionado, limpeza do filtro e aba de detalhes.

## Sexta etapa

- Configurações: componente dedicado `SettingsPage.tsx`, seis grupos com ícones e descrições, painéis separados para vídeo/áudio e visual/abertura, prévia da fonte de captura e banner do cliente original.
- Interface: paleta clara de `Themes/Palette.xaml` (#E9EDF3, #F2F5F9, #DEE5EF e acento #1F6FEB), além de padrão/escuro. Tela inicial configurável entre Início, Coleção, Ranking e Configurações, com opção de abrir em Big Picture na próxima abertura. Os dados anteriores de tema/densidade continuam válidos. A restauração afeta apenas essas preferências, preservando favoritos e dashboard.
- Controle: consulta real via Gamepad API, nomes de dispositivos e indicação de mapeamento padrão/não padrão. Não atribui automaticamente comandos a dispositivos sem mapeamento padrão. Mostra os comandos atuais do Big Picture e o botão nativo de comunicação por voz.
- Armazenamento: consulta da pasta atual por `appDataDir` no desktop, erro tratado e estado próprio para navegador. Atalho para as submissões da sessão. Importação possui atalho para a coleção; captura e gravação mostram os parâmetros atuais somente para consulta.

Validação: build e dez testes aprovados, incluindo migração/validação das preferências. Na prévia, tema claro conferido visualmente e persistente após recarregar; abertura automática em Coleção/Big Picture confirmada; consulta de controles retornou dois dispositivos de mapeamento não padrão; consulta de pasta exibiu a indicação correta de recurso exclusivo do desktop. Tela cheia e pasta real no Tauri ainda precisam de validação nativa. Os dispositivos não padrão exigem uma futura configuração de mapeamento para navegar pelo Big Picture.

## Sétima etapa

- Idioma: seletor Português (Brasil)/English em Configurações > Interface, aplicado imediatamente e salvo com as preferências existentes. Preferências antigas continuam em português. `I18nProvider` atualiza o atributo `lang` do documento e disponibiliza traduções, interpolação de nomes e locale para datas e placares.
- Cobertura: navegação, coleção, sala/detalhes, ranking, eventos, dashboard/personalização, submissões, perfil, assinatura, configurações, ajuda e aviso de expiração. Nomes de jogos, usuários, dispositivos, caminhos e mensagens de chat são mantidos como dados. Mensagens de erro recebidas dos comandos nativos são preservadas no idioma do serviço.
- Valores internos de selects, IDs dos painéis, comandos Tauri e identificadores salvos não são traduzidos. A troca de idioma não remonta a sala ou limpa o rascunho. Datas dos eventos continuam em São Paulo, com formatação correspondente ao idioma.

Validação: build aprovado e treze testes passando, com cobertura das referências estáticas de tradução, interpolação sem alteração dos nomes e validação da preferência de idioma. Na prévia, inglês aplicado imediatamente, persistência após recarregar, datas dos eventos em inglês, filtros funcionando com rótulos traduzidos e retorno ao português preservando jogo selecionado, favoritos e rascunho de chat. Console sem erros durante a verificação.

## Oitava etapa

- Configurar controle: diálogo inspirado em `ConfigureControllerDialog.xaml`, acessível em Configurações > Controle. Lista dispositivos reais, permite capturar botões/eixos para cima, baixo, esquerda, direita, confirmar e voltar, exibe a atribuição e salva/remove o perfil por identificador do dispositivo.
- Validação exige seis ações únicas, índices válidos e direção de eixo válida. A captura espera neutralidade, pausa sem foco, possui timeout e pode ser cancelada. Perfis ficam em `shmupx.client.controller-mappings.v1` e são aplicados ao Big Picture sem reiniciar.
- Big Picture passa a aceitar dispositivos sem mapeamento padrão quando possuem perfil válido. Durante a captura, sua navegação é suspensa e volta a exigir estado neutro ao retomar. A personalização não altera comandos dentro dos jogos ou o push-to-talk nativo.
- Diálogo e mensagens disponíveis em português e inglês, com foco contido e retorno ao acionador. Alterações incompletas não são salvas.

Validação: build aprovado e quinze testes passando, incluindo rejeição de ações duplicadas e tradução de eixos/botões personalizados para os comandos de navegação. Na prévia, dispositivo arcade listado após atualizar controles, botão físico capturado e atribuição exibida; salvamento permaneceu desabilitado com perfil incompleto. Perfil completo e navegação com os seis comandos ainda precisam de validação manual no Tauri. O rascunho de configuração usado na verificação foi descartado.

## Nona etapa

- Biblioteca: `ImportGamesDialog.tsx` porta a composição de `ImportGamesDialog.xaml`, a introdução de conexão de `ConnectPlatformDialog.xaml` e o estado vazio de `LocalGameSelectionDialog.xaml`. O diálogo é acessível pela Coleção e por Configurações > Importar jogos.
- Origens: Steam, Fightcade, emuladores e jogos locais, com descrição de cada fluxo e indicação explícita de conexão indisponível. A seleção local apresenta o estado sem candidatos e mantém a ação de importação desabilitada enquanto não existir serviço que leia a biblioteca.
- Voltar/Esc retornam uma etapa por vez, Fechar encerra o diálogo, o foco permanece no modal e retorna ao acionador. O fluxo compartilha a navegação do Big Picture e as traduções português/inglês.

Validação: build e quinze testes aprovados; na prévia, conferidos abertura pela coleção, origem local, seleção vazia, botão de importação desabilitado, retorno por Esc e restauração do foco. Console sem erros. Esta etapa é visual: não foram criados comandos de sincronização nem importados jogos.

Ainda faltam os serviços necessários para importar bibliotecas, autenticar conta online, sincronizar histórico, inscrever-se em eventos e editar captura/armazenamento. O remapeamento de comandos dentro do jogo não foi portado.

Validação: `npm run build` (TypeScript e Vite) e navegação na prévia local. Confirmados dashboard, detalhe de jogador, filtro sem resultados, catálogo/detalhe de eventos e persistência de tema/coleção compacta após recarregar. O ambiente com Node 20.17 emite aviso de versão mínima do Vite (20.19+ ou 22.12+), mas conclui a compilação. Os comandos nativos precisam de validação manual no aplicativo Tauri com Steam e captura disponíveis.


## Continuação — integração de análise offline

- Ao encerrar a gravação, o Rust chama o script por jogo em `shmupx_offline`, usando `client-v1`; falhas não geram pontuação fictícia.
- A tela de submissões acompanha eventos correlacionados pelo processo de captura: analisando, enviada ou não enviada, com motivo. O retorno do comando de parada sozinho não confirma envio.
- Vídeos usam o ID da run. Timers são associados à run, e uma nova captura é bloqueada durante o processamento para evitar associação ao envio errado no backend atual.
- Ainda não homologado com captura/Steam/backend reais. Catálogo e autenticação continuam demonstrativos; o UUID do Earthion no backend precisa ser associado ao slug do script.
