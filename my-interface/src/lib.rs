use std::collections::HashMap;

use async_trait::async_trait;
use bytes::Bytes;
use dyn_clone::{clone_trait_object, DynClone};
use juniper::{http::GraphQLBatchRequest, Context, DefaultScalarValue, GraphQLEnum};
use warp::{filters::BoxedFilter, http, Filter, Rejection};

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

/// 构造graphql的响应体
pub fn build_response(response: Result<(Vec<u8>, bool), anyhow::Error>) -> http::Response<Vec<u8>> {
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

/// 根据系统获取动态链接包后缀
pub fn get_lib_suffix() -> String {
    match std::env::consts::OS {
        "linux" => "so".to_string(),
        "macos" => "dylib".to_string(),
        "windows" => "dll".to_string(),
        _ => panic!("Unsupport OS"),
    }
}

#[derive(Debug, Clone)]
pub struct Foo {
    pub id: i32,
    pub name: String,
    pub bar_ids: Vec<i32>,
    flag: bool,
}

impl Foo {
    pub fn new(id: i32, name: String, bar_ids: Vec<i32>, flag: bool) -> Self {
        Self {
            id,
            name,
            bar_ids,
            flag,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Bar {
    pub id: i32,
    pub light: Light,
    flag: bool,
}

impl Bar {
    pub fn new(id: i32, light: Light, flag: bool) -> Self {
        Self { id, light, flag }
    }
}

#[derive(GraphQLEnum, Debug, Clone)]
pub enum Light {
    Bright,
    Dark,
}

#[derive(Default, Clone)]
/// 一个模拟数据上下文状态的结构
pub struct DataContext {
    flag: bool,
    foo_storage: HashMap<i32, Foo>,
    bar_storage: HashMap<i32, Bar>,
}

impl Context for DataContext {}

impl DataContext {
    pub fn new() -> Self {
        let mut foo_storage = HashMap::new();
        Self::init_foo(&mut foo_storage);
        let mut bar_storage = HashMap::new();
        Self::init_bar(&mut bar_storage);
        Self {
            flag: false,
            foo_storage: foo_storage,
            bar_storage: bar_storage,
        }
    }
    pub fn flag(&mut self, f: bool) {
        self.flag = f;
    }
    fn init_foo(storage: &mut HashMap<i32, Foo>) {
        storage.insert(1, Foo::new(1, "foo1".to_string(), vec![1, 2], false));
        storage.insert(2, Foo::new(1, "foo2".to_string(), vec![3, 4], false));
        storage.insert(3, Foo::new(3, "foo3".to_string(), vec![5, 6], true));
        storage.insert(4, Foo::new(4, "foo4".to_string(), vec![7, 8], true));
    }
    fn init_bar(storage: &mut HashMap<i32, Bar>) {
        storage.insert(1, Bar::new(1, Light::Bright, false));
        storage.insert(2, Bar::new(2, Light::Dark, false));
        storage.insert(3, Bar::new(3, Light::Bright, false));
        storage.insert(4, Bar::new(4, Light::Dark, false));
        storage.insert(5, Bar::new(5, Light::Bright, true));
        storage.insert(6, Bar::new(6, Light::Dark, false));
        storage.insert(7, Bar::new(7, Light::Bright, false));
        storage.insert(8, Bar::new(8, Light::Dark, true));
    }
    pub fn get_foos(&self) -> Vec<&Foo> {
        self.foo_storage
            .values()
            .filter(|v| v.flag == self.flag)
            .collect()
    }
    pub fn get_foo(&self, id: i32) -> Option<&Foo> {
        self.foo_storage.get(&id)
    }
    pub fn get_bars(&self) -> Vec<&Bar> {
        self.bar_storage
            .values()
            .filter(|v| v.flag == self.flag)
            .collect()
    }
    pub fn get_bar(&self, id: i32) -> Option<&Bar> {
        self.bar_storage.get(&id)
    }
    pub fn get_bars_by_ids(&self, ids: Vec<i32>) -> Vec<&Bar> {
        self.bar_storage
            .values()
            .filter(|v| ids.contains(&v.id) && v.flag == self.flag)
            .collect()
    }
}

pub fn data_context_extractor() -> BoxedFilter<(DataContext,)> {
    warp::any().map(move || DataContext::new()).boxed()
}
