mod error;
mod middleware;
mod models;
mod routes;
mod services;

use std::env;
use std::process::exit;

use axum::Router;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        eprintln!(
            "\n\
             DATABASE_URL não está configurada.\n\n\
             Defina a variável de ambiente com a connection string do Postgres (Supabase ou\n\
             local) antes de rodar o servidor, por exemplo:\n\n  \
             DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres cargo run\n\n\
             Também pode ser definida num arquivo `.env` na raiz de shmupx-backend/.\n"
        );
        exit(1);
    });

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .unwrap_or_else(|err| {
            eprintln!("\nFalha ao conectar no Postgres (DATABASE_URL configurada, mas a conexão falhou): {err}\n");
            exit(1);
        });

    // Permissiva de propósito: ambiente de dev local, liberando o shmupx-frontend (Next.js)
    // e os apps Tauri. Antes de ir para produção, trocar por uma allowlist de origens real.
    let cors = CorsLayer::permissive();

    let app = Router::new()
        .merge(routes::health::router())
        .merge(routes::jogos::router())
        .merge(routes::me::router())
        .merge(routes::rankings::router())
        .merge(routes::runs::router(pool.clone()))
        .with_state(pool)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("falha ao abrir a porta 3000");

    println!("shmupx-backend ouvindo em http://localhost:3000");

    axum::serve(listener, app)
        .await
        .expect("erro ao servir a aplicação Axum");
}
