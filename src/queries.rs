use crate::models;
use bigdecimal::BigDecimal;
use diesel::{
    result::Error, Connection, ExpressionMethods, OptionalExtension, PgConnection, QueryDsl,
    RunQueryDsl,
};

pub enum UserBalance {
    Ok(UserBalanceValues),
    NotFound,
}
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
                    recs.into_iter().fold(BigDecimal::from(0), |acc, rec| {
                        acc + rec.user_currency_value
                    })
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
