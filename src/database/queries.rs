use crate::database::models;
use bigdecimal::BigDecimal;
use diesel::{result::Error, Connection, ExpressionMethods, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl};

#[derive(PartialEq, Debug)]
pub enum UserBalance {
    Ok(UserBalanceValues),
    NotFound,
}
#[derive(PartialEq, Debug)]
pub struct UserBalanceValues {
    pub currency: String,
    pub balance: BigDecimal,
    pub reserved: BigDecimal,
}

pub fn load_balance(conn: &mut PgConnection, req_user_id: &str) -> Result<UserBalance, Error> {
    // wrap in transaction
    conn.transaction::<_, Error, _>(|conn| {
        // load balance
        let balance = {
            use crate::schema::balance::dsl::*;
            balance
                .filter(user_id.eq(req_user_id))
                .first::<models::Balance>(conn)
                .optional()
        };
        let balance = match balance {
            Ok(Some(balance)) => balance,
            Ok(None) => return Ok(UserBalance::NotFound),
            Err(e) => return Err(e),
        };
        // load reserved
        let reserved = {
            use crate::schema::balance_reserve::dsl::*;
            balance_reserve
                .filter(user_id.eq(req_user_id))
                .load::<models::BalanceReserve>(conn)
                .map(|recs| {
                    recs.into_iter()
                        .fold(BigDecimal::from(0), |acc, rec| acc + rec.user_currency_value)
                })
        };
        let reserved = match reserved {
            Ok(reserved) => reserved,
            Err(e) => return Err(e),
        };
        // subtract reserved from balance
        Ok(UserBalance::Ok(UserBalanceValues {
            currency: balance.currency,
            balance: balance.current_value - reserved.clone(),
            reserved,
        }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;
    use crate::database::mutations;
    use bigdecimal::BigDecimal;
    use diesel::result::Error;
    use diesel::Connection;
    use std::ops::DerefMut;

    #[actix_web::test]
    async fn test_load_balance() {
        dotenvy::dotenv().ok();

        let conn = database::connect::create_db_connection_pool();
        let curr = crate::currency::create_currency_converter().await;
        let user_id = "test_load_balance";
        let currency = "USD";
        let value = BigDecimal::from(100);
        let merchant_data = Some("{\"test\": 1}");
        let idempotency_key = "test_load_balance";

        conn.get().unwrap().test_transaction::<_, Error, _>(|conn| {
            // create balance
            let tx_id = mutations::top_up(
                conn.deref_mut(),
                &curr,
                idempotency_key,
                user_id,
                currency,
                value.clone(),
                merchant_data,
            )?;
            assert!(tx_id > 0);
            // load balance
            let balance = load_balance(conn.deref_mut(), user_id)?;
            assert_eq!(
                balance,
                UserBalance::Ok(UserBalanceValues {
                    currency: currency.to_string(),
                    balance: BigDecimal::from(100),
                    reserved: BigDecimal::from(0),
                })
            );
            Ok(())
        });
    }
}
