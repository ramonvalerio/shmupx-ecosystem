# ShmupX Lite na Google Play

Roteiro para preencher o Play Console. Tudo aqui está pronto para copiar e colar.

## 1. Criar o app

Play Console › **Criar app**:

- **Nome do app:** ShmupX Lite
- **Idioma padrão:** Português (Brasil) – pt-BR
- **App ou jogo:** App
- **Gratuito ou pago:** Gratuito
- Marque as duas declarações (Políticas do programa para desenvolvedores e Leis de exportação dos EUA).

## 2. Página da loja (Expandir app › Página principal da loja)

### Português (Brasil)

**Nome:** ShmupX Lite

**Descrição curta** (até 80 caracteres):
> Salas, chat e lives da comunidade ShmupX de shmup, direto no seu celular.

**Descrição completa:**
> O ShmupX Lite é o app de celular da ShmupX, a arena global de shoot 'em up (shmup). Acompanhe a comunidade de onde estiver:
>
> • Entre nas salas de cada jogo do catálogo ShmupX
> • Converse com os jogadores no chat da sala, em tempo real
> • Veja quem está na sala: online, ausente, jogando ou ao vivo
> • Assista às lives dos jogadores direto no app, com o número de espectadores
>
> O ShmupX Lite é a versão leve para acompanhar a plataforma. Para jogar, gravar runs e entrar no ranking, use o ShmupX Client no computador (Windows ou Linux), em shmupx.com.
>
> Entre com sua conta Google: é a mesma conta do ShmupX Client.

### English (en-US), para adicionar como tradução

**Short description:**
> Rooms, chat and live streams from the ShmupX shmup community, on your phone.

**Full description:**
> ShmupX Lite is the mobile app of ShmupX, the global arena of shoot 'em ups (shmups). Follow the community from anywhere:
>
> • Join the room of every game in the ShmupX catalog
> • Chat with players in the room in real time
> • See who is in the room: online, away, playing or live
> • Watch players' live streams right in the app, with the viewer count
>
> ShmupX Lite is the light version for following the platform. To play, record runs and climb the rankings, use the ShmupX Client on your computer (Windows or Linux) at shmupx.com.
>
> Sign in with your Google account, the same one you use in the ShmupX Client.

### Imagens

| Campo | Arquivo |
|---|---|
| Ícone do app (512×512) | `icon-512.png` |
| Recurso gráfico (1024×500) | `feature-1024x500.png` |
| Capturas de tela do telefone (mín. 2) | tire no celular: lista de salas, chat da sala, aba Jogadores (e uma live, se houver) |

### Detalhes de contato e categoria

- **Categoria:** Social
- **E-mail:** shmupx-support@googlegroups.com
- **Site:** https://www.shmupx.com
- **Política de privacidade:** https://www.shmupx.com/privacy

## 3. Conteúdo do app (Política › Conteúdo do app)

**Política de privacidade:** https://www.shmupx.com/privacy

**Anúncios:** Não, meu app não contém anúncios.

**Acesso ao app:**
- Escolha "Todas ou algumas funcionalidades estão restritas".
- Nas instruções, informe: "O app usa Login com o Google. Qualquer conta Google funciona, sem convite nem restrição: toque em Entrar com Google e escolha uma conta."

**Classificação do conteúdo:**
- **Categoria:** "Redes sociais, fóruns, conteúdo gerado por usuários…"
- **Os usuários interagem ou trocam conteúdo?** Sim (chat entre usuários).
- **Compartilha a localização?** Não.
- **Compras digitais?** Não.
- **Violência, sexo, linguagem, drogas, jogos de azar:** Não.

**Público-alvo:**
- **Faixas:** 13–15, 16–17 e 18+.
- **O app é atraente para crianças?** Não.

**Segurança dos dados:**
- **Coleta ou compartilha dados do usuário?** Sim.
- **Os dados são criptografados em trânsito?** Sim.
- **Os usuários podem pedir a exclusão dos dados?** Sim.
- **URL de exclusão da conta:** https://www.shmupx.com/delete-account

Tipos de dados (todos **coletados**, **não compartilhados**, **obrigatórios**):

| Tipo de dado | Finalidades |
|---|---|
| Informações pessoais › **Nome** | Funcionalidade do app; Gerenciamento da conta |
| Informações pessoais › **Endereço de e-mail** | Gerenciamento da conta |
| Informações pessoais › **IDs do usuário** | Funcionalidade do app; Gerenciamento da conta |
| Mensagens › **Outras mensagens no app** (chat da sala) | Funcionalidade do app |
| Atividade no app › **Outras ações** (presença na sala) | Funcionalidade do app |

**Exclusão de conta:** https://www.shmupx.com/delete-account

**App de governo, finanças, saúde, notícias:** Não.

## 4. Primeiro envio (Testes › Teste interno)

1. **Criar nova versão.** Na primeira vez, aceite a **Assinatura de apps do Google Play** com a opção recomendada (o Google gera a chave do app).
2. Envie o arquivo **`ShmupX-Lite-<versão>.aab`**.
3. **Testadores:** crie uma lista com os e-mails das contas Google que vão testar.
4. **Salvar › Revisar › Lançar.** Os testadores recebem um link de convite para instalar pela Play Store.

## 5. Depois do primeiro envio: SHA-1 da chave do Google (obrigatório para o login)

O Google assina o app da loja com uma chave própria. Sem cadastrar essa chave no Firebase, o login com o Google falha no app baixado da Play Store.

1. Vá em **Teste e lançamento › Configuração › Integridade do app › Assinatura de apps**.
2. Copie a **impressão digital do certificado SHA-1** da **chave de assinatura do app**.
3. Passe a SHA-1 para o Claude, que a cadastra no Firebase (é uma informação pública).

## 6. Até a publicação

1. Contas pessoais novas: rode um **teste fechado com pelo menos 12 testadores por 14 dias seguidos** (Testes › Teste fechado).
2. Depois, peça **acesso à produção** no Painel e responda às perguntas sobre o teste.
3. Envie a versão para **Produção**. A revisão leva de horas a uns 7 dias.
