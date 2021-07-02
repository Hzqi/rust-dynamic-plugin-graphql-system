use std::{
    fs::{create_dir_all, remove_dir_all, File},
    io::Write,
};

use proc_macro2::TokenStream;

use crate::errors::BuildError;

/// 创建动态链接包目录
pub fn create_lib_folder_if_not_exist() {
    create_dir_all("./libs").expect("create libs folder failed.")
}

/// 创建临时项目目录
pub fn create_tmp_folder(name: &String) -> Result<(), BuildError> {
    let path = format!("./tmp_{}_project/src", name.to_lowercase());
    create_dir_all(path)
        .map(|_| ())
        .map_err(|e| BuildError::IOError(e))
}

/// 删除临时项目目录
pub fn clean_tmp_folder(name: &String) {
    let path = format!("./tmp_{}_project", name.to_lowercase());
    remove_dir_all(path).expect("clean folder failed.")
}

/// 创建临时项目cargo.toml文件
pub fn create_cargo_toml(name: &String) -> Result<(), BuildError> {
    let lower_name = name.to_lowercase();
    let path = format!("./tmp_{}_project/Cargo.toml", &lower_name);
    let mut file = File::create(path)?;

    let interface_dep_code = r#"{ path = "../my-interface", version = "*" }"#;
    let juniper_dep_code = r#"{version = "0.15.6", features = ["expose-test-schema"]}"#;
    let code = format!(
        r#"
    [package]
    name = "{package_name}-plugin"
    version = "0.1.0"
    authors = ["jackywong@mail.com"]
    edition = "2018"
    
    [workspace]

    [dependencies]
    my-interface = {interface_dep}
    anyhow = "1.0"
    async-trait = "0.1"
    bytes = "1.0.1"
    dyn-clone = "1.0.4"
    juniper = {juniper_dep}
    juniper_warp = "0.6.4"
    warp = "0.3"
    serde = "1.0"
    serde_json = "1.0"
    
    [lib]
    name = "_{lib_name}"
    crate-type = ["dylib"] 
    "#,
        package_name = &lower_name,
        interface_dep = interface_dep_code,
        juniper_dep = juniper_dep_code,
        lib_name = &lower_name
    );
    file.write_all(code.as_bytes())
        .map(|_| ())
        .map_err(|e| BuildError::IOError(e))
}

pub fn create_source(name: &String, tokens: TokenStream) -> Result<(), BuildError> {
    let path = format!("./tmp_{}_project/src/lib.rs", name.to_lowercase());
    let mut file = File::create(path)?;
    file.write_all(tokens.to_string().as_bytes())
        .map(|_| ())
        .map_err(|e| BuildError::IOError(e))
}
