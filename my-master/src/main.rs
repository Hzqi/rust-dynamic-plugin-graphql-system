use std::collections::HashMap;

use libloading::Library;
use my_interface::SayHelloService;
use my_plugin_builder::build_plugin;

fn main() {
    match build_plugin() {
        Ok(_) => println!("[debug] build plugin success"),
        Err(err) => {
            panic!("{}", err);
        }
    }
    let lib = Library::new("libs/libmy_plugin.dylib").expect("load library failed.");
    let create_service: libloading::Symbol<fn() -> Box<dyn SayHelloService>> =
        unsafe { lib.get(b"new_service") }.expect("load symbol");
    let mut hash_map = HashMap::new();
    hash_map.insert("my-plugin-1", create_service());
    hash_map.insert("my-plugin-2", create_service());

    hash_map
        .get("my-plugin-1")
        .and_then(|service| -> Option<()> {
            service.say_hello();
            Some(())
        });
    hash_map.remove("my-plugin-1");
    println!("[debug] only has destroyed one service");

    hash_map
        .get("my-plugin-1")
        .and_then(|service| -> Option<()> {
            service.say_hello();
            Some(())
        });
    println!("[debug] then will destroyed another");
}
