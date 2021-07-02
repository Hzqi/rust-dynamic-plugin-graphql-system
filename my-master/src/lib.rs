use log::error;
use my_interface::GraphqlRequestHandler;
use my_plugin_builder::errors::BuildError;
use std::{collections::HashMap, convert::Infallible};
use warp::{http::StatusCode, Rejection};

pub mod route;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("handler not found")]
    HandlerNotFound,
    #[error("demo not support")]
    DemoNotSupport,
    #[error("load lib error")]
    LoadLibError,
    #[error("load plugin error")]
    LoadPluginError,
    #[error("no such plugin error")]
    NoSuchPluginError,
    #[error(transparent)]
    BuildError(#[from] BuildError),
}

impl warp::reject::Reject for Error {}

pub async fn handle_rejection(err: Rejection) -> std::result::Result<impl warp::Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (StatusCode::NOT_FOUND, "Not Found".to_string())
    } else if let Some(err) = err.find::<Error>() {
        match err {
            Error::HandlerNotFound => (StatusCode::NOT_FOUND, "handler not found".to_string()),
            Error::DemoNotSupport => (StatusCode::BAD_REQUEST, "demo not support".to_string()),
            Error::LoadPluginError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "load plugin error".to_string(),
            ),
            Error::LoadLibError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "load lib error".to_string(),
            ),
            Error::NoSuchPluginError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "no such plugin error".to_string(),
            ),
            Error::BuildError(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e)),
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

/// 处理器的存储容器
#[derive(Clone)]
pub struct HandlerStorage {
    storage: HashMap<String, Box<dyn GraphqlRequestHandler + Send + Sync>>,
}

impl HandlerStorage {
    pub fn new() -> Self {
        let storage = HashMap::new();
        Self { storage }
    }
    pub fn get_handler(
        &self,
        key: String,
    ) -> Option<&Box<dyn GraphqlRequestHandler + Send + Sync>> {
        self.storage.get(&key)
    }
    pub fn has_handler(&self, key: String) -> bool {
        self.storage.contains_key(&key)
    }
    pub fn show_keys(&self) -> Vec<&String> {
        self.storage.keys().collect()
    }
    pub fn add_handler(&mut self, handler: Box<dyn GraphqlRequestHandler + Send + Sync>) {
        self.storage.insert(handler.id(), handler);
    }
    pub fn remove_handler(&mut self, key: String) {
        self.storage.remove(&key);
    }
}
