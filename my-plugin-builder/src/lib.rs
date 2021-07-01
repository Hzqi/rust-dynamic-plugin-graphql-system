use std::{process::Command, str::from_utf8};

use errors::BuildError;
use generate::*;
use my_interface::get_lib_suffix;
use proc_macro2::TokenStream;

pub mod demo;
pub mod errors;
mod generate;

pub fn build_plugin(name: String, tokens: TokenStream) -> Result<(), BuildError> {
    let project_path = format!("./tmp_{}_project", &name.to_lowercase());
    create_lib_folder_if_not_exist();
    create_tmp_folder(&name)?;
    create_cargo_toml(&name)?;
    create_source(&name, tokens)?;

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
    Ok(())
}
