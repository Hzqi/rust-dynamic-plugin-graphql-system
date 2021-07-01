use bytes::Bytes;
use dotenv::dotenv;
use juniper::{http::GraphQLBatchRequest, DefaultScalarValue};
use libloading::Library;
use my_interface::{data_context_extractor, get_lib_suffix, DataContext, GraphqlRequestHandler};
use my_plugin_builder::{build_plugin, demo};
use std::{collections::HashMap, convert::Infallible, net::SocketAddr, path::Path, sync::Arc};
use tokio::sync::{Mutex, MutexGuard};
use warp::{body, http, query, reject, Filter, Rejection, Reply};

use crate::{handle_rejection, Error, HandlerStorage};

type StateContext = Arc<Mutex<HandlerStorage>>;

/// 注入状态上下文context
fn with_context(
    ctx: StateContext,
) -> impl Filter<Extract = (StateContext,), Error = Infallible> + Clone {
    warp::any().map(move || ctx.clone())
}

// 检查否存在相应的动态链接包
fn has_plugin_lib(name: &String) -> bool {
    let path = format!("./libs/lib_{}.{}", name, get_lib_suffix());
    Path::new(&path).exists()
}

// 加载插件
fn load_plugin_to_context(
    path: &String,
    guard: &mut MutexGuard<HandlerStorage>,
) -> Result<(), Error> {
    let lib = Library::new(path).map_err(|e| -> Error {
        log::error!("{}", e);
        Error::LoadLibError
    })?;
    let create_service: libloading::Symbol<fn() -> Box<dyn GraphqlRequestHandler + Send + Sync>> =
        unsafe { lib.get(b"new_service") }.map_err(|e| -> Error {
            log::error!("{}", e);
            Error::LoadPluginError
        })?;
    guard.add_handler(create_service());
    Ok(())
}

// 编译插件
fn create_and_build_plugin(name: &String) -> Result<(), Error> {
    if has_plugin_lib(name) {
        Ok(())
    } else if name == "foo" {
        build_plugin(name.to_owned(), demo::foo::genernate_tokens())?;
        Ok(())
    } else if name == "bar" {
        build_plugin(name.to_owned(), demo::bar::genernate_tokens())?;
        Ok(())
    } else {
        Err(Error::DemoNotSupport)
    }
}

// 在使用时判断载入context
fn load_plugin_on_use(name: &String, guard: &mut MutexGuard<HandlerStorage>) -> Result<(), Error> {
    if guard.has_handler(name.to_owned()) {
        Ok(())
    } else {
        if has_plugin_lib(name) {
            let path = format!("./libs/lib_{}.{}", name, get_lib_suffix());
            load_plugin_to_context(&path, guard)?;
            Ok(())
        } else {
            Err(Error::NoSuchPluginError)
        }
    }
}

async fn build_plugin_handler(name: String) -> Result<impl Reply, Rejection> {
    create_and_build_plugin(&name).map_err(|e| warp::reject::custom(e))?;
    Ok(warp::reply::json(&"ok"))
}

async fn contro_context_handle(
    add_or_remove: String,
    handler_key: String,
    context: StateContext,
) -> Result<impl Reply, Rejection> {
    let mut guard = context.lock().await;
    if add_or_remove == "add" {
        if guard.has_handler(handler_key.clone()) {
            Ok(warp::reply::json(&"already has handler"))
        } else if has_plugin_lib(&handler_key) {
            let path = format!("./libs/lib_{}.{}", &handler_key, get_lib_suffix());
            load_plugin_to_context(&path, &mut guard).map_err(|e| warp::reject::custom(e))?;
            Ok(warp::reply::json(&"ok"))
        } else {
            Err(warp::reject::custom(Error::NoSuchPluginError))
        }
    } else if add_or_remove == "remove" {
        guard.remove_handler(handler_key.to_string());
        Ok(warp::reply::json(&"ok"))
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
    let mut guard = context.lock().await;
    let mut dc = data_context;
    dc.flag(flag);

    load_plugin_on_use(&key, &mut guard).map_err(|e| warp::reject::custom(e))?;
    guard
        .get_handler(key)
        .unwrap()
        .get_request_handle(dc, qry)
        .await
}

async fn graphql_post_json_handler(
    key: String,
    flag: bool,
    context: StateContext,
    data_context: DataContext,
    req: GraphQLBatchRequest<DefaultScalarValue>,
) -> Result<impl Reply, Rejection> {
    let mut guard = context.lock().await;
    let mut dc = data_context;
    dc.flag(flag);

    load_plugin_on_use(&key, &mut guard).map_err(|e| warp::reject::custom(e))?;
    guard
        .get_handler(key)
        .unwrap()
        .post_json_request_handle(dc, req)
        .await
}

async fn graphql_post_graphql_handler(
    key: String,
    flag: bool,
    context: StateContext,
    data_context: DataContext,
    body: Bytes,
) -> Result<impl Reply, Rejection> {
    let mut guard = context.lock().await;
    let mut dc = data_context;
    dc.flag(flag);

    load_plugin_on_use(&key, &mut guard).map_err(|e| warp::reject::custom(e))?;
    guard
        .get_handler(key)
        .unwrap()
        .post_grqphql_request_handle(dc, body)
        .await
}

async fn graphiql_handler(
    key: String,
    flag: bool,
    context: StateContext,
) -> Result<impl Reply, Rejection> {
    let mut guard = context.lock().await;

    load_plugin_on_use(&key, &mut guard).map_err(|e| warp::reject::custom(e))?;
    let graphql_url = format!("/api/{}/graphql/{}", key, flag);
    let html_body =
        juniper::http::graphiql::graphiql_source(graphql_url.as_str(), None).into_bytes();
    let html = http::Response::builder()
        .header("content-type", "text/html;charset=utf-8")
        .body(html_body)
        .expect("response is valid");
    Ok(html)
}

pub async fn run() {
    dotenv().ok();
    pretty_env_logger::init();
    let server_addr = std::env::var("SERVER_ADDR").expect("missing env variable");
    let addr: SocketAddr = server_addr.parse().expect("unable to parse socket address");

    let ctx = Arc::new(Mutex::new(HandlerStorage::new()));

    let home = warp::path::end().map(|| "it works");

    // 编译插件 GET /build/:name
    let build_plugin_route = warp::path!("build" / String)
        .and(warp::get())
        .and_then(build_plugin_handler);

    // 操作处理器存储器 GET /control/:add_or_remove/:name
    let control_context_storage = warp::path!("control" / String / String)
        .and(warp::get())
        .and(with_context(ctx.clone()))
        .and_then(contro_context_handle);

    // Graphql Get请求 GET /api/:name/graphql/:flag
    let graphql_get_route = warp::path!("api" / String / "graphql" / bool)
        .and(warp::get())
        .and(with_context(ctx.clone()))
        .and(data_context_extractor())
        .and(query::query())
        .and_then(graphql_get_handler);

    // Graphql Post json请求 POST /api/:name/graphql/:flag
    let graphql_post_json_route = warp::path!("api" / String / "graphql" / bool)
        .and(warp::post())
        .and(with_context(ctx.clone()))
        .and(data_context_extractor())
        .and(body::json())
        .and_then(graphql_post_json_handler);

    // Graphql POST graphql请求 POST /api/:name/graphql/:flag
    let graphql_post_graphql_route = warp::path!("api" / String / "graphql" / bool)
        .and(warp::post())
        .and(with_context(ctx.clone()))
        .and(data_context_extractor())
        .and(body::bytes())
        .and_then(graphql_post_graphql_handler);

    // Graphiql页面 GET /api/:name/graphiql/:flag
    let graphiql_route = warp::path!("api" / String / "graphiql" / bool)
        .and(warp::get())
        .and(with_context(ctx.clone()))
        .and_then(graphiql_handler);

    let routes = home
        .or(build_plugin_route)
        .or(control_context_storage)
        .or(graphql_get_route)
        .or(graphql_post_json_route)
        .or(graphql_post_graphql_route)
        .or(graphiql_route)
        .recover(handle_rejection);

    warp::serve(routes).run(addr).await
}
