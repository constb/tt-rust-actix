use crate::database::queries::UserBalance;
use actix_web::HttpResponse;
use bigdecimal::Signed;
use prost::Message;

use crate::proto::{error, BadParameterError, Error, GenericOutput, UserBalanceData, UserNotFoundError};

const USER_NOT_FOUND_ERROR: Error = Error {
    one_error: Some(error::OneError::UserNotFound(UserNotFoundError {})),
};

pub fn user_balance_data_http_response(balance: UserBalance, user_id: &str, is_protobuf: bool) -> HttpResponse {
    let data = match balance {
        UserBalance::Ok(balance) => GenericOutput {
            user_balance: Some(UserBalanceData {
                user_id: user_id.to_string(),
                currency: balance.currency,
                value: balance.balance.to_string(),
                reserved_value: balance.reserved.to_string(),
                is_overdraft: balance.balance.is_negative(),
            }),
            ..Default::default()
        },
        UserBalance::NotFound => GenericOutput {
            error: Some(USER_NOT_FOUND_ERROR),
            ..Default::default()
        },
    };
    if is_protobuf {
        HttpResponse::Ok()
            .content_type("application/x-protobuf")
            .body(data.encode_to_vec())
    } else {
        HttpResponse::Ok()
            .content_type("application/json")
            .body(serde_json::to_string(&data).unwrap())
    }
}

pub fn bad_parameter_http_response(field: &str, is_protobuf: bool) -> HttpResponse {
    let data = GenericOutput {
        error: Some(Error {
            one_error: Some(error::OneError::BadParameter(BadParameterError {
                name: field.to_string(),
            })),
        }),
        ..Default::default()
    };
    if is_protobuf {
        HttpResponse::Ok()
            .content_type("application/x-protobuf")
            .body(data.encode_to_vec())
    } else {
        HttpResponse::Ok()
            .content_type("application/json")
            .body(serde_json::to_string(&data).unwrap())
    }
}
