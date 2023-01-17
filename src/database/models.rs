use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Queryable)]
pub struct Balance {
    pub user_id: String,
    pub currency: String,
    pub current_value: BigDecimal,
}

#[derive(Queryable)]
pub struct BalanceReserve {
    pub order_id: String,
    pub user_id: String,
    pub item_id: String,
    pub currency: String,
    pub value: BigDecimal,
    pub user_currency_value: BigDecimal,
    pub created_at: NaiveDateTime,
}

#[derive(Queryable)]
pub struct Transaction {
    pub id: i64,
    pub transaction_currency: String,
    pub transaction_value: BigDecimal,
    pub sender_id: Option<String>,
    pub sender_currency: Option<String>,
    pub sender_value: Option<BigDecimal>,
    pub sender_balance_before: Option<BigDecimal>,
    pub sender_balance_after: Option<BigDecimal>,
    pub recipient_id: Option<String>,
    pub recipient_currency: Option<String>,
    pub recipient_value: Option<BigDecimal>,
    pub recipient_balance_before: Option<BigDecimal>,
    pub recipient_balance_after: Option<BigDecimal>,
    pub merchant_data: Option<serde_json::Value>,
    pub order_data: Option<serde_json::Value>,
    pub created_at: NaiveDateTime,
    pub idempotency_key: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::transaction)]
pub struct NewTopupTransaction {
    pub id: i64,
    pub transaction_currency: String,
    pub transaction_value: BigDecimal,
    pub recipient_id: Option<String>,
    pub recipient_currency: Option<String>,
    pub recipient_value: Option<BigDecimal>,
    pub recipient_balance_before: Option<BigDecimal>,
    pub recipient_balance_after: Option<BigDecimal>,
    pub merchant_data: Option<serde_json::Value>,
    pub created_at: NaiveDateTime,
    pub idempotency_key: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::balance_reserve)]
pub struct NewBalanceReserve {
    pub order_id: String,
    pub user_id: String,
    pub item_id: String,
    pub currency: String,
    pub value: BigDecimal,
    pub user_currency_value: BigDecimal,
    pub created_at: NaiveDateTime,
}
