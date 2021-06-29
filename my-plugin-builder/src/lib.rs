use std::{
    fs::{create_dir_all, remove_dir_all, File},
    io::{Error, Write},
    process::Command,
    str::from_utf8,
};

use proc_macro2::Span;
use quote::quote;

fn create_lib_folder_if_not_exist() {
    create_dir_all("./libs").expect("create libs folder failed.")
}

fn clean_tmp_folder() {
    remove_dir_all("./tmp_cargo_project").expect("clean folder failed.")
}

fn create_tmp_folder() {
    create_dir_all("./tmp_cargo_project/src").expect("create folder failed.")
}

fn create_cargo_toml() {
    let mut file =
        File::create("./tmp_cargo_project/Cargo.toml").expect("create Cargo.toml failed.");
    file.write_all(
        r#"
    [package]
    name = "created-plugin"
    version = "0.1.0"
    authors = ["jackywong@mail.com"]
    edition = "2018"
    
    [workspace]

    [dependencies]
    my-interface = { path = "../my-interface", version = "*" }
    rand = "*"
    
    [lib]
    name = "my_plugin"
    crate-type = ["dylib"] 
    "#
        .as_bytes(),
    )
    .expect("write Cargo.toml failed.")
}

fn create_src() {
    let r#type = syn::Ident::new("MyPluginService", Span::mixed_site());
    let insert_code = quote! {
        #[test]
        fn test_sth() {
            println!("just test.")
        }
    };
    let mut code_quote = quote! {
        use my_interface::SayHelloService;

        #[no_mangle]
        pub fn new_service() -> Box<dyn SayHelloService> {
            Box::new(#r#type::new())
        }

        pub struct #r#type {
            id: String,
        }

        impl #r#type {
            fn new() -> #r#type {
                let id = format!("{:08x}", rand::random::<u32>());
                println!("[{}] Created instance!", id);
                #r#type { id }
            }
        }

        impl SayHelloService for #r#type {
            fn say_hello(&self) {
                println!("[{}] Hello from plugin!", self.id);
            }
        }

        #insert_code
    };
    code_quote.extend(
        quote! {
            impl Drop for #r#type {
                fn drop(&mut self) {
                    println!("[{}] Destroyed instance!", self.id);
                }
            }
        }
        .into_iter(),
    );
    let mut file = File::create("./tmp_cargo_project/src/lib.rs").expect("create src failed.");
    file.write_all(code_quote.to_string().as_bytes())
        .expect("write src failed.")
}

#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error("cargo build error: \n{0}")]
    CargoBuildError(String),
    #[error("move lib error: \n{0}")]
    MoveLibError(String),
    #[error(transparent)]
    IOErrror(#[from] Error),
    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),
}

pub fn build_plugin() -> Result<(), BuildError> {
    use BuildError::*;

    create_lib_folder_if_not_exist();
    create_tmp_folder();
    create_cargo_toml();
    create_src();

    // 编译依赖
    let build_out = Command::new("cargo")
        .current_dir("./tmp_cargo_project")
        .arg("build")
        .output()?;
    if !&build_out.status.success() {
        clean_tmp_folder();
        return Err(CargoBuildError(from_utf8(&build_out.stderr)?.to_string()));
    }

    // 移动动态链接包
    let mv_out = Command::new("mv")
        .current_dir("./tmp_cargo_project")
        .arg("target/debug/libmy_plugin.dylib")
        .arg("../libs")
        .output()?;
    if !&mv_out.status.success() {
        clean_tmp_folder();
        return Err(CargoBuildError(from_utf8(&mv_out.stderr)?.to_string()));
    }
    // 删除临时目录
    clean_tmp_folder();
    Ok(())
}
