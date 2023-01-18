use crate::currency::CurrencyConverter;
use crate::database::models::{NewBalanceReserve, NewTopupTransaction};
use crate::database::{idgen, models};
use bigdecimal::{BigDecimal, FromPrimitive};
use diesel::result::Error;
use diesel::{
    Connection, ExpressionMethods, OptionalExtension, PgAnyJsonExpressionMethods, PgConnection, QueryDsl, RunQueryDsl,
};

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

#[derive(PartialEq, Debug)]
pub enum ReserveResult {
    Ok,
    UserNotFound,
    InsufficientFunds,
    InvalidTransactionState,
}

pub fn reserve(
    conn: &mut PgConnection,
    curr: &CurrencyConverter,
    req_user_id: &str,
    req_currency: &str,
    req_value: BigDecimal,
    req_order_id: &str,
    req_item_id: Option<&str>,
) -> Result<ReserveResult, Error> {
    // wrap in transaction
    conn.transaction::<_, Error, _>(|conn| {
        // load user balance record and lock for update
        let user_balance = {
            use crate::schema::balance::dsl::*;
            balance
                .filter(user_id.eq(req_user_id))
                .for_update()
                .first::<models::Balance>(conn)
                .optional()
        };
        let user_balance: models::Balance = match user_balance {
            Ok(Some(user_balance)) => user_balance,
            Ok(None) => return Ok(ReserveResult::UserNotFound),
            Err(e) => return Err(e),
        };

        // sum existing user's reservations
        let user_reservations = {
            use crate::schema::balance_reserve::dsl::*;
            balance_reserve
                .filter(user_id.eq(req_user_id))
                .select(diesel::dsl::sum(user_currency_value))
                .first::<Option<BigDecimal>>(conn)
                .optional()
        };
        let user_reservations = match user_reservations {
            Ok(Some(user_reservations)) => user_reservations.unwrap_or(BigDecimal::from(0)),
            Ok(None) => BigDecimal::from(0),
            Err(e) => return Err(e),
        };

        // idempotency check (reservation)
        let existing_reservation = {
            use crate::schema::balance_reserve::dsl::*;
            balance_reserve
                .filter(user_id.eq(req_user_id))
                .filter(order_id.eq(req_order_id))
                .first::<models::BalanceReserve>(conn)
                .optional()
        };
        match existing_reservation {
            Ok(Some(_)) => return Ok(ReserveResult::Ok), // already reserved
            Err(e) => return Err(e),
            Ok(None) => {}
        };
        // idempotency check (transaction)
        let existing_transaction = {
            use crate::schema::transaction::dsl::*;
            transaction
                .filter(order_data.retrieve_as_text("order_id").eq(req_order_id))
                .first::<models::Transaction>(conn)
                .optional()
        };
        match existing_transaction {
            Ok(Some(_)) => return Ok(ReserveResult::InvalidTransactionState), // already committed
            Err(e) => return Err(e),
            Ok(None) => {}
        };

        // convert value to user currency
        let reserve_multiplier = if user_balance.currency == req_currency {
            BigDecimal::from(1)
        } else {
            BigDecimal::from_f64(1.06).unwrap()
        };
        let reserve_in_user_currency =
            curr.convert(req_currency, req_value.clone(), user_balance.currency.as_str()) * reserve_multiplier;

        // check if user have enough funds
        if user_balance.current_value.clone() - user_reservations.clone() < reserve_in_user_currency.clone() {
            return Ok(ReserveResult::InsufficientFunds);
        }

        // create reservation record
        {
            use crate::schema::balance_reserve::dsl::*;
            let new_reserve = NewBalanceReserve {
                order_id: req_order_id.to_string(),
                item_id: req_item_id.unwrap_or_default().to_string(),
                user_id: req_user_id.to_string(),
                created_at: chrono::Utc::now().naive_utc(),
                currency: req_currency.to_string(),
                value: req_value,
                user_currency_value: reserve_in_user_currency,
            };
            diesel::insert_into(balance_reserve)
                .values(&new_reserve)
                .execute(conn)?;
        }

        Ok(ReserveResult::Ok)
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
    use diesel::Connection;
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

    #[actix_web::test]
    async fn test_reserve() {
        dotenvy::dotenv().ok();

        let conn = database::connect::create_db_connection_pool();

        let curr = currency::create_currency_converter().await;

        let user_id = "test_user";
        let currency = "USD";
        let value = BigDecimal::from_str("100").unwrap();
        let order_id = "test_order";

        conn.get().unwrap().test_transaction::<_, Error, _>(|conn| {
            let tx_id = top_up(conn, &curr, "id1", user_id, currency, value.clone(), None)?;
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

            let res = reserve(conn, &curr, user_id, currency, value.clone(), order_id, None)?;
            assert_eq!(res, ReserveResult::Ok);

            let balance2 = queries::load_balance(conn, user_id)?;
            assert_eq!(
                balance2,
                UserBalance::Ok(UserBalanceValues {
                    currency: currency.to_string(),
                    balance: BigDecimal::from_str("0").unwrap(),
                    reserved: value.clone()
                })
            );

            Ok(())
        })
    }
}
