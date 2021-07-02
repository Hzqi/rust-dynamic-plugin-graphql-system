use proc_macro2::TokenStream;
use quote::quote;

pub fn genernate_tokens() -> TokenStream {
    let imports = genernate_imports();
    let objects = genernate_objects();
    let graphql_intf = genernate_graphql_intf();
    let handler = genernate_handler();
    quote! {
        #imports
        #objects
        #graphql_intf
        #handler

        #[no_mangle]
        pub fn new_service() -> Box<dyn GraphqlRequestHandler + Send + Sync> {
            Box::new(BarHandler::new())
        }
    }
}

fn genernate_imports() -> TokenStream {
    quote! {
        use anyhow::anyhow;
        use async_trait::async_trait;
        use juniper::{
            futures::FutureExt, graphql_object, http::GraphQLRequest, DefaultScalarValue, EmptyMutation,
            EmptySubscription, FieldResult, RootNode,
        };
        use my_interface::{build_response, Bar, DataContext, Foo, GraphqlRequestHandler, Light};
        use std::{collections::HashMap, marker::Send, sync::Arc};
    }
}

fn genernate_objects() -> TokenStream {
    quote! {
        #[derive(Debug, Clone)]
        pub struct BarObject {
            po: Bar,
        }

        impl From<Bar> for BarObject {
            fn from(po: Bar) -> Self {
                Self { po }
            }
        }

        #[graphql_object(name = "Boo", description = "A Boo Model")]
        impl BarObject {
            fn id(&self) -> i32 {
                self.po.id.clone()
            }
            fn light(&self) -> Light {
                self.po.light.clone()
            }
        }
    }
}

fn genernate_graphql_intf() -> TokenStream {
    quote! {
        pub struct BarQuery;

        #[graphql_object(context = DataContext)]
        impl BarQuery {
            #[graphql(description = "get all bars")]
            fn bars(context: &DataContext) -> Vec<BarObject> {
                context
                    .get_bars()
                    .iter()
                    .map(|po| BarObject::from(po.to_owned().to_owned()))
                    .collect()
            }
            #[graphql(description = "get a bar")]
            fn bar(context: &DataContext, id: i32) -> FieldResult<Option<BarObject>> {
                Ok(context.get_bar(id).map(|po| BarObject::from(po.to_owned())))
            }
        }
    }
}

fn genernate_handler() -> TokenStream {
    quote! {
        #[derive(Clone)]
        pub struct BarHandler<'a> {
            schema: Arc<RootNode<'a, BarQuery, EmptyMutation<DataContext>, EmptySubscription<DataContext>>>,
        }

        impl<'a> BarHandler<'a> {
            pub fn new() -> Self {
                Self {
                    schema: Arc::new(RootNode::new(
                        BarQuery,
                        EmptyMutation::new(),
                        EmptySubscription::new(),
                    )),
                }
            }
        }

        impl<'a> Drop for BarHandler<'a> {
            fn drop(&mut self) {
                println!("Destroyed BarHandler instance!");
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
    }
}
