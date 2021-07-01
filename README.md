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
