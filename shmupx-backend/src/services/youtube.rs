//! YouTube do jogador (spec 033 do Client antigo, migrada): conectar o canal e transmitir ao vivo.
//!
//! A troca do código e a renovação do acesso exigem o segredo do cliente OAuth, que só o servidor
//! conhece; por isso o acesso ao canal (refresh token) fica aqui, em `youtubeConnections/{uid}`
//! (coleção sem regra de leitura: só o backend a vê). O app recebe só o endereço de envio do vídeo
//! (RTMP) e o link da live; nunca o token.
//!
//! Cliente OAuth: `GOOGLE_YOUTUBE_CLIENT_ID/SECRET` (projeto com a YouTube Data API e o escopo
//! `youtube` liberados); sem eles, o mesmo do login do app (`GOOGLE_DESKTOP_*`).

use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::AppError;
use crate::firestore::valor::{data, ler_texto, texto};
use crate::firestore::{Campos, Firestore};

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
const API: &str = "https://www.googleapis.com/youtube/v3";
const ENVIO: &str = "https://www.googleapis.com/upload/youtube/v3";
pub const ESCOPO: &str = "https://www.googleapis.com/auth/youtube";
const COLECAO: &str = "youtubeConnections";
const TITULO_MAXIMO: usize = 100;

pub struct YouTube {
    pub client_id: String,
    secret: String,
    http: reqwest::Client,
}

impl YouTube {
    pub fn do_ambiente() -> Option<Self> {
        let par = |id: &str, secret: &str| {
            let id = std::env::var(id).ok()?.trim().to_string();
            let secret = std::env::var(secret).ok()?.trim().to_string();
            (!id.is_empty() && !secret.is_empty()).then_some((id, secret))
        };
        let (client_id, secret) = par("GOOGLE_YOUTUBE_CLIENT_ID", "GOOGLE_YOUTUBE_CLIENT_SECRET")
            .or_else(|| par("GOOGLE_DESKTOP_CLIENT_ID", "GOOGLE_DESKTOP_CLIENT_SECRET"))?;
        let http = reqwest::Client::builder().timeout(Duration::from_secs(20)).build().ok()?;
        Some(Self { client_id, secret, http })
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Conexao {
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_title: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LiveCriada {
    pub broadcast_id: String,
    pub stream_id: String,
    /// `rtmp://…/live2/<chave>`: segredo da transmissão; o app não o registra em log.
    pub ingest_url: String,
    pub watch_url: String,
}

fn caminho(uid: &str) -> String {
    format!("{COLECAO}/{uid}")
}

/// Erro do Google em mensagem para o jogador (sem dados da conta nem do token).
fn erro_da_api(status: reqwest::StatusCode, corpo: &str) -> AppError {
    let motivo = serde_json::from_str::<Value>(corpo)
        .ok()
        .and_then(|v| v["error"]["errors"][0]["reason"].as_str().map(str::to_string))
        .unwrap_or_default();
    eprintln!("youtube: HTTP {status} ({motivo})");
    match motivo.as_str() {
        "liveStreamingNotEnabled" => AppError::conflict(
            "A transmissão ao vivo não está ativada neste canal. Ative em YouTube Studio › Criar › Transmitir ao vivo (a liberação pode levar até 24 h).",
        ),
        "livePermissionBlocked" => AppError::forbidden("O YouTube bloqueou transmissões ao vivo neste canal."),
        "quotaExceeded" | "dailyLimitExceeded" => AppError::unavailable("Limite diário da API do YouTube atingido; tente amanhã."),
        "insufficientPermissions" => AppError::forbidden("Reconecte o YouTube em Configurações › Integrações para permitir transmissões."),
        _ if status == reqwest::StatusCode::UNAUTHORIZED => {
            AppError::forbidden("Reconecte o YouTube em Configurações › Integrações.")
        }
        _ => AppError::internal(format!("o YouTube recusou o pedido (HTTP {status} {motivo})")),
    }
}

impl YouTube {
    async fn token(&self, form: &[(&str, &str)]) -> Result<Value, AppError> {
        let resposta = self
            .http
            .post(TOKEN_URL)
            .form(form)
            .send()
            .await
            .map_err(|e| AppError::internal(format!("falha de rede com o Google: {e}")))?;
        let status = resposta.status();
        let corpo = resposta.text().await.unwrap_or_default();
        if !status.is_success() {
            eprintln!("youtube: troca de token recusada (HTTP {status})");
            return Err(if status.is_client_error() {
                AppError::forbidden("O Google recusou o acesso ao YouTube; conecte de novo em Configurações › Integrações.")
            } else {
                AppError::internal(format!("Google respondeu HTTP {status}"))
            });
        }
        serde_json::from_str(&corpo).map_err(|e| AppError::internal(format!("resposta do Google inesperada: {e}")))
    }

    async fn api(&self, acesso: &str, metodo: reqwest::Method, caminho: &str, corpo: Option<Value>) -> Result<Value, AppError> {
        let mut pedido = self.http.request(metodo, format!("{API}/{caminho}")).bearer_auth(acesso);
        if let Some(corpo) = corpo {
            pedido = pedido.json(&corpo);
        } else {
            pedido = pedido.header(reqwest::header::CONTENT_LENGTH, "0");
        }
        let resposta = pedido.send().await.map_err(|e| AppError::internal(format!("falha de rede com o YouTube: {e}")))?;
        let status = resposta.status();
        let texto = resposta.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(erro_da_api(status, &texto));
        }
        Ok(if texto.trim().is_empty() { Value::Null } else { serde_json::from_str(&texto).unwrap_or(Value::Null) })
    }

    /// Acesso de curta duração a partir do refresh token guardado.
    async fn acesso(&self, fs: &Firestore, uid: &str) -> Result<String, AppError> {
        let campos = fs.ler_documento(&caminho(uid)).await?.unwrap_or_default();
        let refresh = ler_texto(&campos, "refreshToken")
            .filter(|t| !t.is_empty())
            .ok_or_else(|| AppError::conflict("Conecte seu canal do YouTube em Configurações › Integrações."))?;
        let token = self
            .token(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.secret.as_str()),
                ("refresh_token", refresh.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .await?;
        token["access_token"].as_str().map(str::to_string).ok_or_else(|| AppError::internal("Google não devolveu access_token"))
    }

    pub async fn conexao(&self, fs: &Firestore, uid: &str) -> Result<Conexao, AppError> {
        let campos = fs.ler_documento(&caminho(uid)).await?.unwrap_or_default();
        let conectado = ler_texto(&campos, "refreshToken").is_some_and(|t| !t.is_empty());
        Ok(Conexao { connected: conectado, channel_title: conectado.then(|| ler_texto(&campos, "channelTitle")).flatten() })
    }

    /// Troca o código do consentimento (PKCE) e guarda o acesso ao canal do jogador.
    pub async fn conectar(&self, fs: &Firestore, uid: &str, code: &str, verifier: &str, redirect_uri: &str) -> Result<Conexao, AppError> {
        let token = self
            .token(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.secret.as_str()),
                ("code", code),
                ("code_verifier", verifier),
                ("grant_type", "authorization_code"),
                ("redirect_uri", redirect_uri),
            ])
            .await?;
        let escopos = token["scope"].as_str().unwrap_or_default();
        if !escopos.split(' ').any(|s| s == ESCOPO) {
            return Err(AppError::bad_request("Para transmitir, marque a permissão de gerenciar sua conta do YouTube no Google."));
        }
        let acesso = token["access_token"].as_str().ok_or_else(|| AppError::internal("Google não devolveu access_token"))?;
        // Sem refresh token (o Google só manda no primeiro consentimento), o acesso duraria 1 h.
        let refresh = token["refresh_token"]
            .as_str()
            .ok_or_else(|| AppError::bad_request("O Google não liberou acesso contínuo; desconecte o ShmupX em myaccount.google.com/permissions e conecte de novo."))?;
        let canais = self.api(acesso, reqwest::Method::GET, "channels?part=snippet&mine=true", None).await?;
        let canal = &canais["items"][0];
        let titulo = canal["snippet"]["title"].as_str().unwrap_or("YouTube").to_string();
        if canal["id"].as_str().is_none() {
            return Err(AppError::conflict("Esta conta Google não tem canal no YouTube. Crie um canal e conecte de novo."));
        }

        let mut campos = Campos::new();
        campos.insert("refreshToken".into(), texto(refresh));
        campos.insert("channelId".into(), texto(canal["id"].as_str().unwrap_or_default()));
        campos.insert("channelTitle".into(), texto(&titulo));
        campos.insert("connectedAt".into(), data(Utc::now()));
        fs.gravar(&caminho(uid), campos, &["refreshToken", "channelId", "channelTitle", "connectedAt"]).await?;
        Ok(Conexao { connected: true, channel_title: Some(titulo) })
    }

    /// Esquece o canal e revoga o acesso no Google (melhor esforço).
    pub async fn desconectar(&self, fs: &Firestore, uid: &str) -> Result<(), AppError> {
        let campos = fs.ler_documento(&caminho(uid)).await?.unwrap_or_default();
        if let Some(refresh) = ler_texto(&campos, "refreshToken").filter(|t| !t.is_empty()) {
            let _ = self.http.post(REVOKE_URL).form(&[("token", refresh.as_str())]).send().await;
        }
        let mut vazio = Campos::new();
        vazio.insert("refreshToken".into(), texto(""));
        fs.gravar(&caminho(uid), vazio, &["refreshToken", "channelId", "channelTitle", "connectedAt"]).await
    }

    /// Cria a live (não listada ou pública), o stream RTMP e os liga.
    pub async fn criar_live(&self, fs: &Firestore, uid: &str, titulo: &str, publica: bool) -> Result<LiveCriada, AppError> {
        let titulo: String = titulo.trim().chars().take(TITULO_MAXIMO).collect();
        if titulo.is_empty() {
            return Err(AppError::bad_request("título da live vazio"));
        }
        let acesso = self.acesso(fs, uid).await?;
        let live = self
            .api(
                &acesso,
                reqwest::Method::POST,
                "liveBroadcasts?part=snippet,status,contentDetails",
                Some(json!({
                    "snippet": { "title": titulo, "scheduledStartTime": Utc::now().to_rfc3339() },
                    "status": { "privacyStatus": if publica { "public" } else { "unlisted" }, "selfDeclaredMadeForKids": false },
                    "contentDetails": { "enableAutoStart": false, "enableAutoStop": false, "monitorStream": { "enableMonitorStream": false } },
                })),
            )
            .await?;
        let broadcast_id = live["id"].as_str().ok_or_else(|| AppError::internal("YouTube não devolveu a live"))?.to_string();

        let stream = self
            .api(
                &acesso,
                reqwest::Method::POST,
                "liveStreams?part=snippet,cdn,contentDetails",
                Some(json!({
                    "snippet": { "title": titulo },
                    "cdn": { "ingestionType": "rtmp", "resolution": "variable", "frameRate": "variable" },
                })),
            )
            .await;
        let stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                self.apagar_live(&acesso, &broadcast_id).await;
                return Err(e);
            }
        };
        let stream_id = stream["id"].as_str().unwrap_or_default().to_string();
        let info = &stream["cdn"]["ingestionInfo"];
        let (Some(endereco), Some(chave)) = (info["ingestionAddress"].as_str(), info["streamName"].as_str()) else {
            self.apagar_live(&acesso, &broadcast_id).await;
            return Err(AppError::internal("YouTube não devolveu o endereço de envio"));
        };
        if let Err(e) = self
            .api(
                &acesso,
                reqwest::Method::POST,
                &format!("liveBroadcasts/bind?part=id,contentDetails&id={broadcast_id}&streamId={stream_id}"),
                None,
            )
            .await
        {
            self.apagar_live(&acesso, &broadcast_id).await;
            return Err(e);
        }
        Ok(LiveCriada {
            watch_url: format!("https://www.youtube.com/watch?v={broadcast_id}"),
            ingest_url: format!("{}/{chave}", endereco.trim_end_matches('/')),
            broadcast_id,
            stream_id,
        })
    }

    /// Abre um envio de vídeo (upload retomável) no canal conectado por `uid` e devolve o endereço
    /// do envio. Quem tem o endereço manda os bytes direto ao YouTube, sem o token do canal: é
    /// assim que o Client do jogador posta a live do top 3 no canal oficial da ShmupX.
    pub async fn abrir_envio_de_video(&self, fs: &Firestore, uid: &str, titulo: &str, descricao: &str, tags: &[&str]) -> Result<String, AppError> {
        let titulo: String = titulo.trim().chars().take(TITULO_MAXIMO).collect();
        let descricao: String = descricao.chars().take(4900).collect();
        let acesso = self.acesso(fs, uid).await?;
        let resposta = self
            .http
            .post(format!("{ENVIO}/videos?uploadType=resumable&part=snippet,status"))
            .bearer_auth(&acesso)
            .header("X-Upload-Content-Type", "video/mp4")
            .json(&json!({
                // 20: categoria Jogos.
                "snippet": { "title": titulo, "description": descricao, "categoryId": "20", "tags": tags },
                "status": { "privacyStatus": "public", "selfDeclaredMadeForKids": false },
            }))
            .send()
            .await
            .map_err(|e| AppError::internal(format!("falha de rede com o YouTube: {e}")))?;
        let status = resposta.status();
        let endereco = resposta.headers().get(reqwest::header::LOCATION).and_then(|v| v.to_str().ok()).map(str::to_string);
        if !status.is_success() {
            let corpo = resposta.text().await.unwrap_or_default();
            return Err(erro_da_api(status, &corpo));
        }
        endereco
            .filter(|e| e.starts_with("https://www.googleapis.com/upload/youtube/"))
            .ok_or_else(|| AppError::internal("YouTube não devolveu o endereço do envio"))
    }

    /// Troca o título de um vídeo do canal de `uid` (o VOD da live, com o resultado da run). O
    /// `videos.update` regrava o snippet inteiro: lê o atual e muda só o título, para a descrição,
    /// as tags e a categoria ficarem como estão. `false`: o título já era esse (nada foi gravado).
    pub async fn renomear_video(&self, fs: &Firestore, uid: &str, video_id: &str, titulo: &str) -> Result<bool, AppError> {
        let titulo: String = titulo.trim().chars().take(TITULO_MAXIMO).collect();
        if titulo.is_empty() || !id_valido(video_id) {
            return Err(AppError::bad_request("título ou vídeo inválido"));
        }
        let acesso = self.acesso(fs, uid).await?;
        let atual = self.api(&acesso, reqwest::Method::GET, &format!("videos?part=snippet&id={video_id}"), None).await?;
        let Some(snippet) = atual["items"][0]["snippet"].as_object() else {
            return Err(AppError::not_found("vídeo não encontrado no canal"));
        };
        if snippet.get("title").and_then(Value::as_str) == Some(titulo.as_str()) {
            return Ok(false);
        }
        let mut novo = serde_json::Map::new();
        novo.insert("title".into(), json!(titulo));
        // 20: Jogos, se o vídeo ainda não tiver categoria (o update exige uma).
        novo.insert("categoryId".into(), snippet.get("categoryId").cloned().unwrap_or_else(|| json!("20")));
        for campo in ["description", "tags", "defaultLanguage"] {
            if let Some(valor) = snippet.get(campo) {
                novo.insert(campo.into(), valor.clone());
            }
        }
        self.api(&acesso, reqwest::Method::PUT, "videos?part=snippet", Some(json!({ "id": video_id, "snippet": novo }))).await?;
        Ok(true)
    }

    async fn apagar_live(&self, acesso: &str, broadcast_id: &str) {
        let _ = self.api(acesso, reqwest::Method::DELETE, &format!("liveBroadcasts?id={broadcast_id}"), None).await;
    }

    /// `active` quando o YouTube já recebe o vídeo; antes disso, `ready`/`inactive`.
    pub async fn situacao_do_stream(&self, fs: &Firestore, uid: &str, stream_id: &str) -> Result<String, AppError> {
        let acesso = self.acesso(fs, uid).await?;
        let resposta = self.api(&acesso, reqwest::Method::GET, &format!("liveStreams?part=status&id={stream_id}"), None).await?;
        Ok(resposta["items"][0]["status"]["streamStatus"].as_str().unwrap_or("unknown").to_string())
    }

    /// Pessoas assistindo agora à live do próprio jogador (`concurrentViewers`; 0 fora do ar).
    pub async fn espectadores(&self, fs: &Firestore, uid: &str, broadcast_id: &str) -> Result<u64, AppError> {
        let acesso = self.acesso(fs, uid).await?;
        let resposta = self.api(&acesso, reqwest::Method::GET, &format!("videos?part=liveStreamingDetails&id={broadcast_id}"), None).await?;
        Ok(contar_espectadores(&resposta))
    }

    /// Coloca no ar (`live`) ou encerra (`complete`). Encerrar uma live que nunca foi ao ar a apaga.
    pub async fn transicao(&self, fs: &Firestore, uid: &str, broadcast_id: &str, para: Transicao) -> Result<(), AppError> {
        let acesso = self.acesso(fs, uid).await?;
        let estado = match para {
            Transicao::Live => "live",
            Transicao::Complete => "complete",
        };
        let resultado = self
            .api(
                &acesso,
                reqwest::Method::POST,
                &format!("liveBroadcasts/transition?part=status&broadcastStatus={estado}&id={broadcast_id}"),
                None,
            )
            .await;
        match (resultado, para) {
            (Ok(_), _) => Ok(()),
            // Nunca entrou no ar: não dá para "completar"; some com ela.
            (Err(_), Transicao::Complete) => {
                self.apagar_live(&acesso, broadcast_id).await;
                Ok(())
            }
            (Err(e), Transicao::Live) => Err(e),
        }
    }
}

/// `concurrentViewers` vem como texto na API do YouTube (e some quando a live não está no ar).
fn contar_espectadores(resposta: &Value) -> u64 {
    let campo = &resposta["items"][0]["liveStreamingDetails"]["concurrentViewers"];
    campo.as_u64().or_else(|| campo.as_str().and_then(|v| v.parse().ok())).unwrap_or(0)
}

#[cfg(test)]
mod testes_espectadores {
    use super::*;

    #[test]
    fn conta_espectadores_em_texto_numero_ou_ausente() {
        assert_eq!(contar_espectadores(&serde_json::json!({ "items": [{ "liveStreamingDetails": { "concurrentViewers": "42" } }] })), 42);
        assert_eq!(contar_espectadores(&serde_json::json!({ "items": [{ "liveStreamingDetails": { "concurrentViewers": 7 } }] })), 7);
        assert_eq!(contar_espectadores(&serde_json::json!({ "items": [] })), 0);
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Transicao {
    Live,
    Complete,
}

/// Id do YouTube (live/stream): letras, números, `-` e `_`. Barra o uso das rotas para montar
/// outras URLs da API.
pub fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_validos() {
        assert!(id_valido("dQw4w9WgXcQ"));
        assert!(!id_valido("x&part=snippet"));
        assert!(!id_valido(""));
    }

    #[test]
    fn erros_da_api_viram_mensagens() {
        let corpo = r#"{"error":{"errors":[{"reason":"liveStreamingNotEnabled"}]}}"#;
        assert!(matches!(erro_da_api(reqwest::StatusCode::FORBIDDEN, corpo), AppError::Conflict(_)));
        assert!(matches!(erro_da_api(reqwest::StatusCode::UNAUTHORIZED, "{}"), AppError::Forbidden(_)));
    }
}
