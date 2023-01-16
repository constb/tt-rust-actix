use std::env;

use actix_request_identifier::{IdReuse, RequestIdentifier};
use actix_web::web::Data;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

use crate::routes::{balance_handler, top_up_handler};

mod currency;
mod idgen;
mod models;
mod mutations;
mod proto;
mod queries;
mod responses;
mod routes;
mod schema;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[actix_web::main]
async fn main() {
    dotenvy::dotenv().ok();

    // setup tracing and use bunyan formatter
    let formatting_layer = BunyanFormattingLayer::new("tt-rust".into(), std::io::stdout);
    let subscriber = Registry::default()
        .with(filter_fn(|metadata| {
            *metadata.level() <= tracing::Level::INFO
        }))
        .with(JsonStorageLayer)
        .with(formatting_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let db = create_db_connection_pool();
    run_migrations(&db);

    let currency_converter = currency::create_currency_converter().await;

    let server = actix_web::HttpServer::new(move || {
        let db = db.clone();

        actix_web::App::new()
            .wrap(RequestIdentifier::with_uuid().use_incoming_id(IdReuse::UseIncoming))
            .wrap(actix_web::middleware::Logger::default())
            .app_data(Data::new(db.clone()))
            .app_data(Data::new(currency_converter.clone()))
            .service(balance_handler)
            .service(top_up_handler)
    });

    server
        .bind(env::var("BIND_ADDRESS").unwrap())
        .unwrap()
        .run()
        .await
        .unwrap();
}

// create database connection pool with the database url using diesel
fn create_db_connection_pool() -> Pool<ConnectionManager<PgConnection>> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::builder()
        .test_on_check_out(true)
        .build(manager)
        .expect("Failed to create db connection pool.")
}

// run diesel migrations
fn run_migrations(pool: &Pool<ConnectionManager<PgConnection>>) -> () {
    pool.get()
        .unwrap()
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
}
