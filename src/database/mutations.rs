use crate::currency::CurrencyConverter;
use crate::database::models::NewTopupTransaction;
use crate::database::{idgen, models};
use bigdecimal::BigDecimal;
use diesel::result::Error;
use diesel::{Connection, ExpressionMethods, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl};

// creates new balance table record, on conflict does nothing
fn init_user_balance(conn: &mut PgConnection, req_currency: &str, req_user_id: &str) -> Result<bool, Error> {
    use crate::schema::balance::dsl::*;
    diesel::insert_into(balance)
        .values((
            user_id.eq(req_user_id),
            currency.eq(req_currency),
            current_value.eq(BigDecimal::from(0)),
        ))
        .on_conflict(user_id)
        .do_nothing()
        .execute(conn)
        .map(|res| res > 0)
}

// adds value to balance, returns new balance
pub fn top_up(
    conn: &mut PgConnection,
    curr: &CurrencyConverter,
    req_idempotency_key: &str,
    req_user_id: &str,
    req_currency: &str,
    req_value: BigDecimal,
    req_merchant_data: Option<&str>,
) -> Result<i64, Error> {
    init_user_balance(conn, req_currency, req_user_id)?;

    // wrap in transaction
    conn.transaction::<_, Error, _>(|conn| {
        // load user balance record and lock for update
        let user_balance = {
            use crate::schema::balance::dsl::*;
            balance
                .filter(user_id.eq(req_user_id))
                .for_update()
                .first::<models::Balance>(conn)
        };
        let user_balance: models::Balance = match user_balance {
            Ok(user_balance) => user_balance,
            Err(e) => return Err(e),
        };
        // idempotency check
        let user_transaction = {
            use crate::schema::transaction::dsl::*;
            transaction
                .filter(idempotency_key.eq(req_idempotency_key))
                .first::<models::Transaction>(conn)
                .optional()
        };
        match user_transaction {
            Ok(Some(user_transaction)) => return Ok(user_transaction.id),
            Err(e) => return Err(e),
            Ok(None) => {}
        };

        // convert value to user currency
        let topup_in_user_currency = curr.convert(req_currency, req_value.clone(), user_balance.currency.as_str());
        let balance_after_topup = user_balance.current_value.clone() + topup_in_user_currency.clone();

        let tx_id = idgen::next();
        {
            // create transaction record
            use crate::schema::transaction::dsl::*;

            let new_transaction = NewTopupTransaction {
                id: tx_id,
                transaction_currency: req_currency.to_string(),
                transaction_value: req_value,
                recipient_id: Some(req_user_id.to_string()),
                recipient_currency: Some(user_balance.currency.to_string()),
                recipient_value: Some(topup_in_user_currency),
                recipient_balance_before: Some(user_balance.current_value.clone()),
                recipient_balance_after: Some(balance_after_topup.clone()),
                merchant_data: req_merchant_data.map(|s| serde_json::Value::String(s.to_string())),
                created_at: chrono::Utc::now().naive_utc(),
                idempotency_key: Some(req_idempotency_key.to_string()),
            };
            diesel::insert_into(transaction)
                .values(&new_transaction)
                .execute(conn)?;
        }
        {
            // update balance
            use crate::schema::balance::dsl::*;
            diesel::update(balance.filter(user_id.eq(req_user_id)))
                .set(current_value.eq(balance_after_topup))
                .execute(conn)?;
        }

        // return new transaction id
        return Ok(tx_id);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::queries;
    use crate::database::queries::{UserBalance, UserBalanceValues};
    use crate::{currency, database};
    use bigdecimal::BigDecimal;
    use diesel::result::Error;
    use diesel::{Connection, ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl};
    use std::str::FromStr;

    #[actix_web::test]
    async fn test_top_up() {
        dotenvy::dotenv().ok();

        let conn = database::connect::create_db_connection_pool();

        let curr = currency::create_currency_converter().await;

        let user_id = "test_user";
        let currency = "USD";
        let value = BigDecimal::from_str("100").unwrap();
        let idempotency_key = "test";

        conn.get().unwrap().test_transaction::<_, Error, _>(|conn| {
            let tx_id = top_up(conn, &curr, idempotency_key, user_id, currency, value.clone(), None)?;
            assert!(tx_id > 0);

            let balance = queries::load_balance(conn, user_id)?;
            assert_eq!(
                balance,
                UserBalance::Ok(UserBalanceValues {
                    currency: currency.to_string(),
                    balance: value.clone(),
                    reserved: Default::default()
                })
            );

            let tx_id2 = top_up(conn, &curr, idempotency_key, user_id, currency, value.clone(), None)?;
            assert_eq!(tx_id, tx_id2);

            let balance2 = queries::load_balance(conn, user_id)?;
            assert_eq!(balance2, balance);

            Ok(())
        });
    }
}
