# rust-dynamic-plugin-system

这个是warp + grapqhql(juniper) + 动态编译warp的handler的一个demo

目录结构：

* my-interface: 插件包的依赖接口、通用依赖等
* my-plugin-builder: 插件包的生成模块，用于在运行时动态构建项目进行编译
* my-master: 主服务

http接口如下：

* `GET localhost:8080/build/:name` 进行动态编译操作，本demo的name仅有`foo`、`bar`
* `GET localhost:8080/control/:action/:name` 进行动态新增/减少handler存储器中的handler。action: `add` `remove`，本demo的name仅有`foo`、`bar`
* `GET/POST localhost:8080/api/:name/graphql/:flag`  Graphql的接口，有三种方式GET、POST json、POST graphql。通过`:name`去区分不同的接口。
  * 其中`:flag`是用于区分同一种graphql接口中的不同数据范畴，如同一个数据在`flag = false`的数据: `localhost:8080/api/:name/graphql/false` (主要是用于验证并模拟在不同环境下的接口操作，如`master`、`dev`)
  * 本demo的name仅有`foo`、`bar`。当插件包已经编译，但是未加载到context中时，会自动加载，不需要手动触发
* `GET localhost:8080/api/:name/graphiql/:flag` Graphiql客户端页面，接口处理逻辑与graphql的一样



# Documentation

详细说明本demo的各个模块的实现，主要分为两个部分：动态Graphql接口处理器、动态编译插件。



## 动态Graphql接口处理器

依赖包`juniper_warp`中其实有提供到生成直接使用的warp filter，但是这个生成的filter是静态的，绑定到了route上，而无法做到动态替换。 所以使用的方案是自行实现一个通用的filter，在filter的`and_then`中直接提供graphql所需的参数，处理响应。

GET请求的handle函数大致如下：

```rust
async fn get_request_handle(
    context: DataContext,
    mut qry: HashMap<String, String>,
) -> Result<warp::http::Response<Vec<u8>>, warp::Rejection> {
    let schema = ... ; // Graphql实现的schema结构，类型为RootNode<'a, QueryT, MutationT, SubscriptionT>
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

```

其中的`req.execute(&schema, &context).await;`、`build_response(res)`是`juniper_warp`包所提供的函数，直接生成响应。

Graphql的POST json接口、POST graphql接口都是类似，直接实现处理函数，然后就能在filter的`and_then`中指定，如GET请求的filter：

```rust
let graphql_get_route = warp::path!("api" / "graphql")
	.and(warp::get())
	.and(data_context_extractor())
	.and(query::query())
	.and_then(get_request_handle);
```



### GraphqlRequestHandler特型

要实现动态去选择graphql的handler，就需要一个在server中间状态的context中存储对应的处理方式。为了更方便的获取处理handler和处理，就需要定义一个特型，并在context中存储该特型的实例。

另外，因为该特型中所提供的http处理函数是异步的，所以整个特型需要`#[async_trait]`的支持。这个特型的实例需要能够Clone，因此也需要`dyn_clone::Clone`的支持（能自动去实现特型对象的相关Clone）。特型代码大致如下：

```rust
/// 请求处理器的特型
#[async_trait]
pub trait GraphqlRequestHandler: DynClone {
    fn id(&self) -> String;
  	/// 处理GET的Graphql接口
    async fn get_request_handle(
        &self,
        context: DataContext,
        qry: HashMap<String, String>,
    ) -> Result<http::Response<Vec<u8>>, Rejection>;
    /// 处理POST的Graphql接口，参数类型是application/json
    async fn post_json_request_handle(
        &self,
        context: DataContext,
        req: GraphQLBatchRequest<DefaultScalarValue>,
    ) -> Result<http::Response<Vec<u8>>, Rejection>;
    /// 处理POST的Graphql接口，参数类型是application/graphql
    async fn post_grqphql_request_handle(
        &self,
        context: DataContext,
        body: Bytes,
    ) -> Result<http::Response<Vec<u8>>, Rejection>;
}
clone_trait_object!(GraphqlRequestHandler);
```



### HandlerStorage

处理器存储器，用于存储GraphqlRequestHandler实现对象的容器，本demo是简单的演示、验证demo，因此本容器会比较简单，仅是一个`HashMap`的结构。

```rust
/// 处理器的存储容器
#[derive(Clone)]
pub struct HandlerStorage {
    storage: HashMap<String, Box<dyn GraphqlRequestHandler + Send + Sync>>,
}
```

 由于该容器是需要在server中作为一个状态属性的存在，要提供可以增删的功能，因此在warp的服务中是一个`Arc`引用，同时因为需要读、增、删，该属性必须为可修改的变量，因为又涉及到请求时多个线程间的共享，该属性必须为锁属性。对于处理器存储器来说，是读的可能性比写的更多，这里使用读写锁来确保安全性和并发性。

```rust
let ctx = Arc::new(RwLock::new(HandlerStorage::new()));
```

（这里使用的`RwLock`是`tokio::sync::RwLock`的结构）

每个filter都需要这个处理器的存储状态，因此需要将其传入filter里：

```rust
type StateContext = Arc<RwLock<HandlerStorage>>;

/// 注入状态上下文context
fn with_context(
    ctx: StateContext,
) -> impl Filter<Extract = (StateContext,), Error = Infallible> + Clone {
    warp::any().map(move || ctx.clone())
}
```



注意到，每一个graphql的处理逻辑都需要一个`DataContext`的信息，该信息就是Graphql底层数据处理的逻辑提供方，在实际的项目中，该`DataContext`一般是数据库的连接器，提供底层数据库的操作，本demo仅是一个简单的fake结构。

因为每一个处理都需要这个DataContext，因此要把它注入到filter中：

```rust
fn data_context_extractor() -> BoxedFilter<(DataContext,)> {
    warp::any().map(move || DataContext::new()).boxed()
}
```



在准备好处理器特型、处理器后，就可以在warp filter的`and_then`通过参数去选择对应的Graphql处理：

```rust
async fn graphql_get_handler(
	key: String,
	context: StateContext,
	data_context: DataContext,
	qry: HashMap<String, String>,
) -> Result<impl Reply, Rejection> {
  // 获取处理器容器的可读引用
	let read_guard = context.read().await;
  // 通过参数去获取对应的处理器，并处理提供逻辑
	match read_guard.get_handler(key) {
      Some(handler) => handler.get_request_handle(data_context, qry).await,
    	None => Err(warp::reject())
  }
}
```

然后对应的route就类似如下：

```rust
// 从URL path veritable中指定选择对应的处理器
let graphql_get_route = warp::path!("api" / String / "graphql")
	.and(warp::get())
	.and(with_context(ctx.clone()))
	.and(data_context_extractor())
	.and(query::query())
	.and_then(graphql_get_handler);
```

在请求时，就可以根据URL Path参数的不同而选择不同的Graphql接口。



当要新增处理器时，就需要对处理器容器进行写锁：

```rust
async fn add_context_handler(context: StateContext, handler: Box<dyn GraphqlRequestHandler + Send + Sync>) {
  let mut write_guard = context.write().await;
  write_guard.add_handler(handler);
}
```

当有很多请求时，只有新增处理器时才会对其他请求有阻塞等待。



## 动态编译插件

在运行时编译代码成动态链接包的方式其实只需要两个步骤：1、生成临时项目及源码， 2、编译临时项目。



### 生成临时项目

直接通过`File`包来创建目录和文件，并写入文件。写如文件重点时两个：`{temp_project}/Cargo.toml`和`{temp_project}/src/lib.rs`。

该临时项目，必须要有整个项目的接口包作为依赖，使得编译后能与主服务交互。同时也需要指定编译类型为`dylib`来编译成系统的动态链接包：

```toml
[dependencies]
my-interface = {path = "...", version = "*"}
# ... other dependencies

[lib]
crate-type = ["dylib"] 
```



### 生成逻辑代码

为了更方便的生成项目与代码，动态编译插件的部分，将所有逻辑都写入`src/lib.rs`文件。而生成逻辑代码部分，是使用`quote`依赖包的`quote!`宏来写代码，这样可以说的写生成代码时像直接写代码一样：

```rust
fn genernate_code -> TokenStream {
  quote! {
    fn say_hello() -> String {
      format!("Hello {}", #outer_variable)
    }
  }
}
```

使用`quote`可以通过`#变量名`的方式来注入外部的信息。最后通过转成字符串来写入文件：

```rust
pub fn create_source(name: &String, tokens: TokenStream) -> Result<(), BuildError> {
    let path = format!("./tmp_{}_project/src/lib.rs", name.to_lowercase());
    let mut file = File::create(path)?;
    file.write_all(tokens.to_string().as_bytes())
        .map(|_| ())
        .map_err(|e| BuildError::IOError(e))
}
```

需要注意，生成的代码中必须包含一个创建实例的函数，该函数是用来被主服务读取动态链接包后所调用的。一般来说创建的实例是接口包里逻辑特型的实现实例：

```rust
quote! {
  #[no_mangle]
  pub fn new_service() -> Box<dyn GraphqlRequestHandler + Send + Sync> {
		Box::new(GenernatedHandler::new())
	}
}
```



### 编译临时项目

上述步骤中就已经完成生成整个项目了，接下来就是进行编译。编译的处理非常简单，就是通过调用系统命令的`cargo build`，并把编译成功后的动态链接包移动到主服务可以访问到的目录（一般整个项目统一处理的）：

```rust
    // 编译依赖
    let build_out = Command::new("cargo")
        .current_dir(&project_path)
        .arg("build")
        .output()?;
    if !&build_out.status.success() {
        clean_tmp_folder(&name);
        log::error!("{}", from_utf8(&build_out.stderr).unwrap());
        return Err(BuildError::BuildProjectError(name));
    }

    let target_suffix = get_lib_suffix();
    let target_path = format!(
        "target/debug/lib_{}.{}",
        &name.to_lowercase(),
        target_suffix
    );
    // 移动动态链接包
    let mv_out = Command::new("mv")
        .current_dir(&project_path)
        .arg(target_path)
        .arg("../libs")
        .output()?;
    if !&mv_out.status.success() {
        clean_tmp_folder(&name);
        log::error!("{}", from_utf8(&mv_out.stderr).unwrap());
        return Err(BuildError::MoveLibError(name));
    }
    // 删除临时目录
    clean_tmp_folder(&name);
```

编译完成后，动态链接包就放置在`./libs`中，且临时创建的目录会被删除。



## 动态Graphql与动态编译集成

集成部分，就是在主服务中，加载动态编译成功的动态链接包，创建GraphqlRequestHandler的实例，然后放进HandlerStorage里即可：

```rust
// 加载插件
fn load_plugin_to_context(
    path: &String,
    guard: &mut RwLockWriteGuard<HandlerStorage>,
) -> Result<(), Error> {
    let lib = Library::new(path).map_err(|e| Error::LoadLibError)?;
    let create_service: libloading::Symbol<fn() -> Box<dyn GraphqlRequestHandler + Send + Sync>> =
        unsafe { lib.get(b"new_service") }.map_err(|e| Error::LoadPluginError)?;
    guard.add_handler(create_service());
    Ok(())
}
```



本Demo中就提供了两个动态生成的代码`foo`、`bar`，分别对应两个动态的Graphql接口。
