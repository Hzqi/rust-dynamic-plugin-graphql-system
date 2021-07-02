#[derive(thiserror::Error, Debug)]
pub enum BuildError {
    #[error("create temporary project folder failed: \n{0}")]
    CreateProjectFolderError(String),
    #[error("create temporary project cargo.toml failed: \n{0}")]
    CreateCargoTomlError(String),
    #[error("create temporary project src failed: \n{0}")]
    CreateSrcError(String),
    #[error("build temporary project failed: \n{0}")]
    BuildProjectError(String),
    #[error("move lib error: \n{0}")]
    MoveLibError(String),
    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}
