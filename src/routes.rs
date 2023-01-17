#![allow(unused_variables)]

use std::ops::DerefMut;
use std::str::FromStr;

use actix_request_identifier::RequestId;
use actix_web::{get, http::header, post, web, HttpResponse};
use bigdecimal::{BigDecimal, Signed, Zero};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use tracing::{error, instrument};

use crate::currency::CurrencyConverter;
use crate::database::{mutations, queries};
use crate::{proto, responses};

#[get("/balance/{user_id}")]
#[instrument(skip(db), fields(request_id = request_id.as_str()))]
pub async fn balance_handler(
    db: web::Data<Pool<ConnectionManager<PgConnection>>>,
    request_id: RequestId,
    accept: web::Header<header::Accept>,
    user_id: web::Path<String>,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let user_id = user_id.clone();
    let is_protobuf = accept
        .iter()
        .any(|a| a.to_string() == "application/x-protobuf".to_string());

    let mut conn = db.get()?;

    let user_id1 = user_id.clone();
    web::block(move || queries::load_balance(conn.deref_mut(), user_id1.as_str()).map_err(anyhow::Error::from))
        .await
        .unwrap_or_else(|e| {
            error!("{e}");
            Err(e.into())
        })
        .map(|balance| responses::user_balance_data_http_response(balance, user_id.as_str(), is_protobuf))
        .map_err(Into::into)
}

#[post("/top-up")]
#[instrument(skip(db), fields(request_id = request_id.as_str()))]
pub async fn top_up_handler(
    db: web::Data<Pool<ConnectionManager<PgConnection>>>,
    curr: web::Data<CurrencyConverter>,
    request_id: RequestId,
    accept: web::Header<header::Accept>,
    top_up_request: web::Json<proto::TopUpInput>,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let is_protobuf = accept
        .iter()
        .any(|a| a.to_string() == "application/x-protobuf".to_string());

    let mut conn = db.get()?;

    if top_up_request.idempotency_key.is_empty() {
        return Ok(responses::bad_parameter_http_response("idempotency_key", is_protobuf));
    }
    if top_up_request.user_id.is_empty() {
        return Ok(responses::bad_parameter_http_response("user_id", is_protobuf));
    }
    if !curr.is_currency_valid(&top_up_request.currency) {
        return Ok(responses::bad_parameter_http_response("currency", is_protobuf));
    }

    let req_value = BigDecimal::from_str(top_up_request.value.as_str());
    let req_value = match req_value {
        Ok(req_value) => req_value,
        Err(_) => return Ok(responses::bad_parameter_http_response("value", is_protobuf)),
    };
    if req_value.is_negative() || req_value.is_zero() {
        return Ok(responses::bad_parameter_http_response("value", is_protobuf));
    }

    // if merchant data is not empty, check if it is valid json
    if !top_up_request.merchant_data.is_empty() {
        let json = serde_json::from_str::<serde_json::Value>(top_up_request.merchant_data.as_str());
        if json.is_err() {
            return Ok(responses::bad_parameter_http_response("merchant_data", is_protobuf));
        }
    }

    let user_id1 = top_up_request.user_id.clone();
    web::block(move || {
        let req_merchant_data = if top_up_request.merchant_data.is_empty() {
            None
        } else {
            Some(top_up_request.merchant_data.as_str())
        };
        mutations::top_up(
            conn.deref_mut(),
            &curr,
            top_up_request.idempotency_key.as_str(),
            top_up_request.user_id.as_str(),
            top_up_request.currency.as_str(),
            req_value,
            req_merchant_data,
        )
        .map_err(anyhow::Error::from)?;
        queries::load_balance(conn.deref_mut(), top_up_request.user_id.as_str()).map_err(anyhow::Error::from)
    })
    .await
    .unwrap_or_else(|e| {
        error!("{e}");
        Err(e.into())
    })
    .map(|balance| responses::user_balance_data_http_response(balance, user_id1.as_str(), is_protobuf))
    .map_err(Into::into)
}
