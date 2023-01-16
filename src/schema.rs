// @generated automatically by Diesel CLI.

diesel::table! {
    balance (user_id) {
        user_id -> Varchar,
        currency -> Varchar,
        current_value -> Numeric,
    }
}

diesel::table! {
    balance_reserve (order_id) {
        order_id -> Varchar,
        user_id -> Varchar,
        item_id -> Varchar,
        currency -> Varchar,
        value -> Numeric,
        user_currency_value -> Numeric,
        created_at -> Timestamp,
    }
}

diesel::table! {
    transaction (id) {
        id -> Int8,
        transaction_currency -> Varchar,
        transaction_value -> Numeric,
        sender_id -> Nullable<Varchar>,
        sender_currency -> Nullable<Varchar>,
        sender_value -> Nullable<Numeric>,
        sender_balance_before -> Nullable<Numeric>,
        sender_balance_after -> Nullable<Numeric>,
        recipient_id -> Nullable<Varchar>,
        recipient_currency -> Nullable<Varchar>,
        recipient_value -> Nullable<Numeric>,
        recipient_balance_before -> Nullable<Numeric>,
        recipient_balance_after -> Nullable<Numeric>,
        merchant_data -> Nullable<Jsonb>,
        order_data -> Nullable<Jsonb>,
        created_at -> Timestamp,
        idempotency_key -> Nullable<Varchar>,
    }
}

diesel::joinable!(balance_reserve -> balance (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    balance,
    balance_reserve,
    transaction,
);
