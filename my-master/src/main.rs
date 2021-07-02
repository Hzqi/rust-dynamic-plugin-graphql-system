// use libloading::Library;
// use my_interface::SayHelloService;
// use my_plugin_builder::build_plugin;

/// Runs the API server.
#[tokio::main]
async fn main() {
    my_master::route::run().await
}
