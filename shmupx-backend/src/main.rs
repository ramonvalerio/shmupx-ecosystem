mod error;
mod firestore;
mod middleware;
mod models;
mod routes;
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

#[tokio::main]
async fn main() {
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

    let mut app = Router::new()
        .merge(routes::health::router())
        .merge(routes::auth::router(google))
        .merge(routes::me::router())
        .merge(routes::admin::router(firestore.clone()))
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
