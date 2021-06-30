use bytes::Bytes;
use dotenv::dotenv;
use juniper::{http::GraphQLBatchRequest, DefaultScalarValue};
use std::{collections::HashMap, convert::Infallible, net::SocketAddr, sync::Arc};

use tokio::sync::Mutex;
use warp::{body, http, query, reject, Filter, Rejection, Reply};

use crate::{
    handle_rejection,
    implement::{data_context_extractor, DataContext, HandlerStorage},
    Error,
};

type StateContext = Arc<Mutex<HandlerStorage>>;

/// 注入状态上下文context
fn with_context(
    ctx: StateContext,
) -> impl Filter<Extract = (StateContext,), Error = Infallible> + Clone {
    warp::any().map(move || ctx.clone())
}

async fn contro_context_handle(
    add_or_remove: String,
    foo_or_bar: String,
    context: StateContext,
) -> Result<impl Reply, Rejection> {
    let mut guard = context.lock().await;
    if add_or_remove == "add" {
        if foo_or_bar == "foo" {
            guard.add_foo();
            Ok(warp::reply::json(&"ok"))
        } else if foo_or_bar == "bar" {
            guard.add_bar();
            Ok(warp::reply::json(&"ok"))
        } else {
            Err(reject())
        }
    } else if add_or_remove == "remove" {
        if foo_or_bar == "foo" {
            guard.remove_foo();
            Ok(warp::reply::json(&"ok"))
        } else if foo_or_bar == "bar" {
            guard.remove_bar();
            Ok(warp::reply::json(&"ok"))
        } else {
            Err(reject())
        }
    } else {
        Err(reject())
    }
}

async fn graphql_get_handler(
    key: String,
    flag: bool,
    context: StateContext,
    data_context: DataContext,
    qry: HashMap<String, String>,
) -> Result<impl Reply, Rejection> {
    let guard = context.lock().await;
    let mut dc = data_context;
    dc.flag(flag);
    match guard.get_handler(key) {
        Some(handler) => handler.get_request_handle(dc, qry).await,
        None => Err(warp::reject::custom(Error::HandlerNotFound)),
    }
}

async fn graphql_post_json_handler(
    key: String,
    flag: bool,
    context: StateContext,
    data_context: DataContext,
    req: GraphQLBatchRequest<DefaultScalarValue>,
) -> Result<impl Reply, Rejection> {
    let guard = context.lock().await;
    let mut dc = data_context;
    dc.flag(flag);
    match guard.get_handler(key) {
        Some(handler) => handler.post_json_request_handle(dc, req).await,
        None => Err(warp::reject::custom(Error::HandlerNotFound)),
    }
}

async fn graphql_post_graphql_handler(
    key: String,
    flag: bool,
    context: StateContext,
    data_context: DataContext,
    body: Bytes,
) -> Result<impl Reply, Rejection> {
    let guard = context.lock().await;
    let mut dc = data_context;
    dc.flag(flag);
    match guard.get_handler(key) {
        Some(handler) => handler.post_grqphql_request_handle(dc, body).await,
        None => Err(warp::reject::custom(Error::HandlerNotFound)),
    }
}

async fn graphiql_handler(
    key: String,
    flag: bool,
    context: StateContext,
) -> Result<impl Reply, Rejection> {
    let guard = context.lock().await;
    if guard.has_handler(key.clone()) {
        let graphql_url = format!("/api/{}/graphql/{}", key, flag);
        let html_body =
            juniper::http::graphiql::graphiql_source(graphql_url.as_str(), None).into_bytes();
        let html = http::Response::builder()
            .header("content-type", "text/html;charset=utf-8")
            .body(html_body)
            .expect("response is valid");
        Ok(html)
    } else {
        Err(warp::reject::custom(Error::HandlerNotFound))
    }
}

pub async fn run() {
    dotenv().ok();
    pretty_env_logger::init();
    let server_addr = std::env::var("SERVER_ADDR").expect("missing env variable");
    let addr: SocketAddr = server_addr.parse().expect("unable to parse socket address");

    let ctx = Arc::new(Mutex::new(HandlerStorage::new()));

    let home = warp::path::end().map(|| "it works");

    let control_context_storage = warp::path!("control" / String / String)
        .and(warp::get())
        .and(with_context(ctx.clone()))
        .and_then(contro_context_handle);

    let graphql_get_route = warp::path!("api" / String / "graphql" / bool)
        .and(warp::get())
        .and(with_context(ctx.clone()))
        .and(data_context_extractor())
        .and(query::query())
        .and_then(graphql_get_handler);

    let graphql_post_json_route = warp::path!("api" / String / "graphql" / bool)
        .and(warp::post())
        .and(with_context(ctx.clone()))
        .and(data_context_extractor())
        .and(body::json())
        .and_then(graphql_post_json_handler);

    let graphql_post_graphql_route = warp::path!("api" / String / "graphql" / bool)
        .and(warp::post())
        .and(with_context(ctx.clone()))
        .and(data_context_extractor())
        .and(body::bytes())
        .and_then(graphql_post_graphql_handler);

    let graphiql_route = warp::path!("api" / String / "graphiql" / bool)
        .and(warp::get())
        .and(with_context(ctx.clone()))
        .and_then(graphiql_handler);

    let routes = home
        .or(control_context_storage)
        .or(graphql_get_route)
        .or(graphql_post_json_route)
        .or(graphql_post_graphql_route)
        .or(graphiql_route)
        .recover(handle_rejection);

    warp::serve(routes).run(addr).await
}
