# Verificação do app OAuth no Google (escopo do YouTube)

Projeto `shmupx-project-id` (dono: `shmupx.adm@gmail.com`), Google Auth Platform. Sem a verificação,
quem conecta o YouTube vê "O Google não verificou este app" e o app fica limitado a 100 contas.

## Checklist

| Item | Onde | Situação |
|---|---|---|
| App name `ShmupX` | Branding | feito |
| User support email `shmupx-support@googlegroups.com` | Branding | feito |
| Home page `https://www.shmupx.com` | Branding | feito |
| Privacy policy `https://www.shmupx.com/privacy` | Branding + site | página criada (publicar o site) |
| Terms of service `https://www.shmupx.com/terms` | Branding + site | página criada; preencher o link no Branding |
| Authorized domain `shmupx.com` | Branding | feito |
| Domínio `shmupx.com` verificado no Search Console, pela conta `shmupx.adm@gmail.com` | search.google.com/search-console | pendente |
| Logo 120×120 (opcional; enviar logo também exige verificação) | Branding | opcional |
| Escopo `https://www.googleapis.com/auth/youtube` | Data Access | feito |
| Justificativa do escopo | Data Access | texto abaixo |
| Vídeo de demonstração (YouTube, não listado) | Data Access › Demo video | roteiro abaixo |
| Enviar para revisão | Verification Center | depois de tudo acima |

### Search Console

1. Em search.google.com/search-console, com `shmupx.adm@gmail.com`: **Adicionar propriedade → Domínio → `shmupx.com`**.
2. O Google mostra um registro TXT (`google-site-verification=...`). Criar esse TXT na raiz de `shmupx.com`, no DNS
   do domínio (Route 53, pelo `shmupx-infra`).
3. Clicar em **Verificar**. A propagação do DNS pode levar alguns minutos.

## Justificativa do escopo ("How will the scopes be used?")

> ShmupX Client is a desktop app for shoot 'em up players. When a player clicks "Start live" in a game room, the
> app creates a live broadcast on the player's own YouTube channel (liveBroadcasts.insert, liveStreams.insert,
> liveBroadcasts.bind), streams the game screen to it and ends it when the player clicks "End live", when the
> stream stops or after 45 minutes (liveBroadcasts.transition). It also reads the channel name
> (channels.list mine=true) to show which channel is connected. Broadcasts are unlisted unless the player ticks
> "Public live". The youtube.readonly scope cannot create or end broadcasts and youtube.upload cannot manage live
> broadcasts, so the youtube scope is the narrowest scope that allows live streaming. ShmupX never uploads, edits
> or deletes videos and only acts after an explicit action of the player. The refresh token is stored only on our
> server and can be revoked from the app (Settings > Integrations > Disconnect).

## Roteiro do vídeo de demonstração (2 a 3 min, em inglês)

O Google precisa ver: o nome do app na tela de consentimento, o escopo pedido, e o uso real de cada permissão no
app. Gravar a tela inteira (navegador e app), sem cortes no consentimento. Narração ou legendas em inglês.

1. **Abertura (10 s).** Mostrar o site `https://www.shmupx.com`, rolar até o rodapé e abrir **Privacy Policy**.
2. **App (10 s).** Abrir o ShmupX Client já logado. Legenda: "ShmupX Client — desktop app for shmup players".
3. **Conectar (40 s).** Settings › Integrations › YouTube › **Connect**.
   - No navegador, mostrar a **barra de endereço** com o `client_id` do projeto `230979440400`.
   - Escolher a conta e o canal.
   - Na tela de consentimento, mostrar o **nome ShmupX** e a permissão **"Manage your YouTube account"**; clicar em
     Continue.
   - Voltar ao app: o cartão mostra **"Connected as ShmupX"** (uso do `channels.list`).
4. **Transmitir (60 s).** Abrir uma sala (Collection › duplo clique no jogo).
   - Mostrar a caixa **Public live** desmarcada e clicar em **Start live**.
   - Mostrar "Creating the live…" e depois **LIVE** na barra de cima (`liveBroadcasts.insert`, `liveStreams.insert`,
     `bind`, `transition` para live).
   - Clicar em **Open on YouTube**: a live aparece no canal, não listada, com a tela do jogo.
5. **Encerrar (20 s).** Clicar em **End live**; no YouTube Studio a transmissão aparece como encerrada
   (`transition` para complete).
6. **Revogar (20 s).** Settings › Integrations › **Disconnect**: o cartão volta para "Not connected". Opcional:
   mostrar myaccount.google.com/permissions sem o ShmupX.

Subir o vídeo no YouTube como **não listado** e colar o link no campo do Data Access.

## Depois do envio

O Google responde por e-mail ao `shmupx.adm@gmail.com` (developer contact). Pedidos comuns: ajustar o texto da
política, mostrar no vídeo o `client_id` e a tela de consentimento em inglês. A revisão do escopo sensível costuma
levar de alguns dias a algumas semanas.
