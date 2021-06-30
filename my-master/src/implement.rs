use anyhow::anyhow;
use async_trait::async_trait;
use juniper::{
    futures::FutureExt, graphql_object, http::GraphQLRequest, Context, DefaultScalarValue,
    EmptyMutation, EmptySubscription, GraphQLEnum, GraphQLInputObject, GraphQLObject, RootNode,
};
use std::{collections::HashMap, marker::Send, sync::Arc};
use warp::{filters::BoxedFilter, Filter};

use crate::{build_response, GraphqlRequestHandler};

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
    pub fn add_foo(&mut self) {
        self.storage
            .insert("foo".to_string(), Box::new(FooHandler::new()));
    }
    pub fn remove_foo(&mut self) {
        self.storage.remove(&"foo".to_string());
    }
    pub fn add_bar(&mut self) {
        self.storage
            .insert("bar".to_string(), Box::new(BarHandler::new()));
    }
    pub fn remove_bar(&mut self) {
        self.storage.remove(&"bar".to_string());
    }
}

#[derive(GraphQLObject, Clone, Debug)]
#[graphql(description = "A Foo model")]
pub struct Foo {
    id: i32,
    name: String,
}

#[derive(GraphQLInputObject)]
#[graphql(description = "A input struct to create Foo")]
struct FooCreateInput {
    id: i32,
    name: String,
}

#[derive(GraphQLEnum, Clone, Debug)]
enum Light {
    Green,
    Yellow,
    Red,
}

#[derive(GraphQLObject, Clone, Debug)]
#[graphql(description = "A Bar model")]
pub struct Bar {
    id: i32,
    light: Light,
}

#[derive(Default, Clone)]
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
        storage.insert(
            1,
            Foo {
                id: 1,
                name: String::from("foo1"),
            },
        );
        storage.insert(
            2,
            Foo {
                id: 2,
                name: String::from("foo2"),
            },
        );
    }
    fn init_bar(storage: &mut HashMap<i32, Bar>) {
        storage.insert(
            1,
            Bar {
                id: 1,
                light: Light::Green,
            },
        );
        storage.insert(
            2,
            Bar {
                id: 2,
                light: Light::Yellow,
            },
        );
    }
}

/// graphql 上下文构造器
pub fn data_context_extractor() -> BoxedFilter<(DataContext,)> {
    warp::any().map(move || DataContext::new()).boxed()
}

pub struct FooQuery {}
pub struct FooMutation {}
pub struct BarQuery {}

#[graphql_object(context = DataContext)]
impl FooQuery {
    #[graphql(description = "get all foos")]
    fn foos(context: &DataContext) -> Vec<&Foo> {
        if context.flag {
            context
                .foo_storage
                .values()
                .into_iter()
                .filter(|v| v.id < 2)
                .collect()
        } else {
            context.foo_storage.values().into_iter().collect()
        }
    }
}

#[graphql_object(context = DataContext)]
impl FooMutation {
    #[graphql(description = "create a foo (update context will not success.)")]
    fn add_foo(_context: &DataContext, data: FooCreateInput) -> Option<Foo> {
        let foo = Foo {
            id: data.id,
            name: data.name,
        };
        Some(foo)
    }
}

#[graphql_object(context = DataContext)]
impl BarQuery {
    #[graphql(description = "get all bars")]
    fn bars(context: &DataContext) -> Vec<&Bar> {
        if context.flag {
            context
                .bar_storage
                .values()
                .into_iter()
                .filter(|v| v.id < 2)
                .collect()
        } else {
            context.bar_storage.values().into_iter().collect()
        }
    }
}

// fn foo_schema<'a>(
// ) -> RootNode<'a, FooQuery, EmptyMutation<DataContext>, EmptySubscription<DataContext>> {
//     RootNode::new(FooQuery {}, EmptyMutation::new(), EmptySubscription::new())
// }

// fn bar_schema<'a>(
// ) -> RootNode<'a, BarQuery, EmptyMutation<DataContext>, EmptySubscription<DataContext>> {
//     RootNode::new(BarQuery {}, EmptyMutation::new(), EmptySubscription::new())
// }

#[derive(Clone)]
struct FooHandler<'a> {
    schema: Arc<RootNode<'a, FooQuery, FooMutation, EmptySubscription<DataContext>>>,
}

impl<'a> FooHandler<'a> {
    pub fn new() -> Self {
        Self {
            schema: Arc::new(RootNode::new(
                FooQuery {},
                FooMutation {},
                EmptySubscription::new(),
            )),
        }
    }
}

#[async_trait]
impl<'a> GraphqlRequestHandler for FooHandler<'a> {
    fn id(&self) -> String {
        String::from("foo")
    }

    async fn get_request_handle(
        &self,
        context: DataContext,
        mut qry: HashMap<String, String>,
    ) -> Result<warp::http::Response<Vec<u8>>, warp::Rejection> {
        let schema = self.schema.clone();
        async move {
            let req = GraphQLRequest::new(
                qry.remove("query")
                    .ok_or_else(|| anyhow!("Missing GraphQL query string in query parameters"))?,
                qry.remove("operation_name"),
                qry.remove("variables")
                    .map(|vs| serde_json::from_str(&vs))
                    .transpose()?,
            );

            let resp = req.execute(&schema, &context).await;

            Ok((serde_json::to_vec(&resp)?, resp.is_ok()))
        }
        .then(|res| async move { Ok::<_, warp::Rejection>(build_response(res)) })
        .await
    }

    async fn post_json_request_handle(
        &self,
        context: DataContext,
        req: juniper::http::GraphQLBatchRequest<DefaultScalarValue>,
    ) -> Result<warp::http::Response<Vec<u8>>, warp::Rejection> {
        let schema = self.schema.clone();
        async move {
            let resp = req.execute(&schema, &context).await;

            Ok::<_, warp::Rejection>(build_response(
                serde_json::to_vec(&resp)
                    .map(|json| (json, resp.is_ok()))
                    .map_err(Into::into),
            ))
        }
        .await
    }

    async fn post_grqphql_request_handle(
        &self,
        context: DataContext,
        body: bytes::Bytes,
    ) -> Result<warp::http::Response<Vec<u8>>, warp::Rejection> {
        let schema = self.schema.clone();
        async move {
            let query = std::str::from_utf8(body.as_ref())
                .map_err(|e| anyhow!("Request body query is not a valid UTF-8 string: {}", e))?;
            let req = GraphQLRequest::new(query.into(), None, None);

            let resp = req.execute(&schema, &context).await;

            Ok((serde_json::to_vec(&resp)?, resp.is_ok()))
        }
        .then(|res| async { Ok::<_, warp::Rejection>(build_response(res)) })
        .await
    }
}

#[derive(Clone)]
struct BarHandler<'a> {
    schema: Arc<RootNode<'a, BarQuery, EmptyMutation<DataContext>, EmptySubscription<DataContext>>>,
}

impl<'a> BarHandler<'a> {
    pub fn new() -> Self {
        Self {
            schema: Arc::new(RootNode::new(
                BarQuery {},
                EmptyMutation::new(),
                EmptySubscription::new(),
            )),
        }
    }
}

#[async_trait]
impl<'a> GraphqlRequestHandler for BarHandler<'a> {
    fn id(&self) -> String {
        String::from("bar")
    }

    async fn get_request_handle(
        &self,
        context: DataContext,
        mut qry: HashMap<String, String>,
    ) -> Result<warp::http::Response<Vec<u8>>, warp::Rejection> {
        let schema = self.schema.clone();
        async move {
            let req = GraphQLRequest::new(
                qry.remove("query")
                    .ok_or_else(|| anyhow!("Missing GraphQL query string in query parameters"))?,
                qry.remove("operation_name"),
                qry.remove("variables")
                    .map(|vs| serde_json::from_str(&vs))
                    .transpose()?,
            );

            let resp = req.execute(&schema, &context).await;

            Ok((serde_json::to_vec(&resp)?, resp.is_ok()))
        }
        .then(|res| async move { Ok::<_, warp::Rejection>(build_response(res)) })
        .await
    }

    async fn post_json_request_handle(
        &self,
        context: DataContext,
        req: juniper::http::GraphQLBatchRequest<DefaultScalarValue>,
    ) -> Result<warp::http::Response<Vec<u8>>, warp::Rejection> {
        let schema = self.schema.clone();
        async move {
            let resp = req.execute(&schema, &context).await;

            Ok::<_, warp::Rejection>(build_response(
                serde_json::to_vec(&resp)
                    .map(|json| (json, resp.is_ok()))
                    .map_err(Into::into),
            ))
        }
        .await
    }

    async fn post_grqphql_request_handle(
        &self,
        context: DataContext,
        body: bytes::Bytes,
    ) -> Result<warp::http::Response<Vec<u8>>, warp::Rejection> {
        let schema = self.schema.clone();
        async move {
            let query = std::str::from_utf8(body.as_ref())
                .map_err(|e| anyhow!("Request body query is not a valid UTF-8 string: {}", e))?;
            let req = GraphQLRequest::new(query.into(), None, None);

            let resp = req.execute(&schema, &context).await;

            Ok((serde_json::to_vec(&resp)?, resp.is_ok()))
        }
        .then(|res| async { Ok::<_, warp::Rejection>(build_response(res)) })
        .await
    }
}
