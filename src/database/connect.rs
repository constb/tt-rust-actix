use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::env;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

// create database connection pool with the database url using diesel
pub fn create_db_connection_pool() -> Pool<ConnectionManager<PgConnection>> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::builder()
        .test_on_check_out(true)
        .build(manager)
        .expect("Failed to create db connection pool.")
}

// run diesel migrations
pub fn run_migrations(pool: &Pool<ConnectionManager<PgConnection>>) -> () {
    pool.get()
        .unwrap()
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
}
