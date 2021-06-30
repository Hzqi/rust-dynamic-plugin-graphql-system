use async_trait::async_trait;
use bytes::Bytes;
use dyn_clone::{clone_trait_object, DynClone};
use implement::DataContext;
use juniper::{http::GraphQLBatchRequest, DefaultScalarValue};
use log::error;
use std::{collections::HashMap, convert::Infallible};
use tokio::task;
use warp::{
    http::{self, StatusCode},
    Rejection,
};

pub mod implement;
pub mod route;

/// 请求处理器的特型
#[async_trait]
pub trait GraphqlRequestHandler: DynClone {
    fn id(&self) -> String;
    async fn get_request_handle(
        &self,
        context: DataContext,
        qry: HashMap<String, String>,
    ) -> Result<http::Response<Vec<u8>>, Rejection>;
    async fn post_json_request_handle(
        &self,
        context: DataContext,
        req: GraphQLBatchRequest<DefaultScalarValue>,
    ) -> Result<http::Response<Vec<u8>>, Rejection>;
    async fn post_grqphql_request_handle(
        &self,
        context: DataContext,
        body: Bytes,
    ) -> Result<http::Response<Vec<u8>>, Rejection>;
}
clone_trait_object!(GraphqlRequestHandler);

#[derive(Debug)]
pub struct JoinError(task::JoinError);

impl warp::reject::Reject for JoinError {}

fn build_response(response: Result<(Vec<u8>, bool), anyhow::Error>) -> http::Response<Vec<u8>> {
    match response {
        Ok((body, is_ok)) => http::Response::builder()
            .status(if is_ok { 200 } else { 400 })
            .header("content-type", "application/json")
            .body(body)
            .expect("response is valid"),
        Err(_) => http::Response::builder()
            .status(http::StatusCode::INTERNAL_SERVER_ERROR)
            .body(Vec::new())
            .expect("status code is valid"),
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("handler not found")]
    HandlerNotFound,
}

impl warp::reject::Reject for Error {}

pub async fn handle_rejection(err: Rejection) -> std::result::Result<impl warp::Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (StatusCode::NOT_FOUND, "Not Found".to_string())
    } else if let Some(err) = err.find::<Error>() {
        match err {
            Error::HandlerNotFound => (StatusCode::NOT_FOUND, "handler not found".to_string()),
        }
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        (
            StatusCode::METHOD_NOT_ALLOWED,
            "Method Not Allowed".to_string(),
        )
    } else {
        error!("unhandled error: {:?}", err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error".to_string(),
        )
    };

    Ok(warp::reply::with_status(message, code))
}
