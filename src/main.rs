use std::env;

use actix_request_identifier::{IdReuse, RequestIdentifier};
use actix_web::web::Data;

use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

use crate::database::connect::{create_db_connection_pool, run_migrations};
use crate::routes::{balance_handler, commit_handler, reserve_handler, top_up_handler};

mod currency;
mod database;
mod proto;
mod responses;
mod routes;
mod schema;

#[actix_web::main]
async fn main() {
    dotenvy::dotenv().ok();

    // setup tracing and use bunyan formatter
    let formatting_layer = BunyanFormattingLayer::new("tt-rust".into(), std::io::stdout);
    let subscriber = Registry::default()
        .with(filter_fn(|metadata| *metadata.level() <= tracing::Level::INFO))
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
            .service(reserve_handler)
            .service(commit_handler)
    });

    server
        .bind(env::var("BIND_ADDRESS").unwrap())
        .unwrap()
        .run()
        .await
        .unwrap();
}
