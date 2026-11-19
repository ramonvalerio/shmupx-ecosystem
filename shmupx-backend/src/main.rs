mod error;
mod firestore;
mod middleware;
mod models;
mod routes;
mod segredos_aws;
mod services;

use std::env;
use std::process::exit;
use std::sync::Arc;

use axum::Router;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::firestore::Firestore;
use crate::routes::auth::GoogleOAuth;
use crate::services::email::Ses;

fn main() {
    // Segredos de produção (AWS Secrets Manager) entram no ambiente antes de qualquer outra thread
    // existir: `set_var` só é seguro com um único thread, por isso o runtime principal vem depois.
    if let Some(cfg) = segredos_aws::Configuracao::do_ambiente() {
        let valores = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime para ler os segredos")
            .block_on(segredos_aws::carregar(&cfg))
            .unwrap_or_else(|erro| {
                eprintln!("\nFalha ao ler os segredos na AWS: {erro}\n");
                exit(1);
            });
        let mut aplicados = Vec::new();
        for (nome, valor) in valores {
            if env::var_os(&nome).is_none() && !valor.is_empty() {
                // SAFETY: ainda não há outras threads; o runtime acima já terminou.
                unsafe { env::set_var(&nome, valor) };
                aplicados.push(nome);
            }
        }
        println!("segredos carregados da AWS: {}", aplicados.join(", "));
    }

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime do tokio")
        .block_on(servidor());
}

async fn servidor() {
    // Emulador de login (tokens sem assinatura) só em desenvolvimento local: ver middleware/firebase.rs.
    if let Err(erro) = middleware::firebase::conferir_ambiente_do_emulador() {
        eprintln!("\nConfiguração recusada: {erro}\n");
        exit(1);
    }
    if middleware::firebase::emulador_de_login_ativo() {
        println!("AMBIENTE DE DESENVOLVIMENTO: Firestore e login no emulador local (projeto {}).", env::var("FIREBASE_PROJECT_ID").unwrap_or_default());
    }
    // Firestore é o banco da plataforma (ADR 005): fichas e gameplays.
    let firestore = Arc::new(Firestore::do_ambiente().unwrap_or_else(|erro| {
        eprintln!(
            "
             Firestore não configurado: {erro}

             Defina FIREBASE_PROJECT_ID e, em produção, GOOGLE_APPLICATION_CREDENTIALS com o caminho
             do JSON da conta de serviço (fora do repositório). Para o emulador local, defina
             FIRESTORE_EMULATOR_HOST=127.0.0.1:8088.
"
        );
        exit(1);
    }));

    // Permissiva de propósito: ambiente de dev local, liberando o shmupx-frontend (Next.js)
    // e os apps Tauri. Antes de ir para produção, trocar por uma allowlist de origens real.
    let cors = CorsLayer::permissive();

    // Troca do código de login do app desktop: o segredo do cliente OAuth fica só aqui.
    let google = GoogleOAuth::do_ambiente().map(Arc::new);
    if google.is_none() {
        eprintln!(
            "aviso: GOOGLE_DESKTOP_CLIENT_ID/GOOGLE_DESKTOP_CLIENT_SECRET ausentes; o login do app desktop não vai funcionar"
        );
    }

    // Avisos de versão por e-mail (Amazon SES): opcional; sem configuração, só o aviso no Client.
    let email = Ses::do_ambiente().map(Arc::new);
    if email.is_none() {
        eprintln!("aviso: EMAIL_FROM/AWS_SECRETS_ROLE_ARN ausentes (SES não configurado); os avisos de versão não serão enviados por e-mail");
    }

    let mut app = Router::new()
        .merge(routes::health::router())
        .merge(routes::auth::router(google))
        .merge(routes::me::router())
        .merge(routes::admin::router(firestore.clone(), email.clone()))
        .merge(routes::mister::router(firestore.clone(), email))
        .merge(routes::notificacoes::router(firestore.clone()))
        .merge(routes::notificacoes::router_publico(firestore.clone()))
        .merge(routes::ranking_publico::router(firestore.clone()))
        .merge(routes::runs::router(firestore));

    // Legado: catálogo e ranking do portal ainda leem o PostgreSQL/Supabase. Só são montados
    // quando DATABASE_URL existe; o fluxo do cliente não depende deles.
    if let Ok(database_url) = env::var("DATABASE_URL") {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(&database_url)
            .await
            .unwrap_or_else(|err| {
                eprintln!("
Falha ao conectar no Postgres (DATABASE_URL configurada, mas a conexão falhou): {err}
");
                exit(1);
            });
        app = app.merge(
            Router::new()
                .merge(routes::jogos::router())
                .merge(routes::rankings::router())
                .with_state(pool),
        );
    }

    let app = app.layer(cors).layer(TraceLayer::new_for_http());

    // O Cloud Run informa a porta em PORT; localmente o padrão continua 3000.
    let porta = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let listener = TcpListener::bind(format!("0.0.0.0:{porta}"))
        .await
        .unwrap_or_else(|err| panic!("falha ao abrir a porta {porta}: {err}"));

    println!("shmupx-backend ouvindo em http://localhost:{porta}");

    axum::serve(listener, app)
        .await
        .expect("erro ao servir a aplicação Axum");
}
