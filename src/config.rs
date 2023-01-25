use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use grace::GraceCompilerOptions;
use sarzak_mc::SarzakCompilerOptions;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub domains: HashMap<String, DomainConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DomainConfig {
    /// Path to the model file
    ///
    pub path: PathBuf,
    /// Name of the generated module
    ///
    /// Defaults to the name of the domain.
    pub module: String,
    /// The compiler to use for this domain
    ///
    pub compiler: Compiler,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "compiler")]
#[serde(rename_all = "lowercase")]
pub enum Compiler {
    /// Sarzak Model Compiler
    ///
    /// This is the first model compiler, based off of nut. It generates domain
    /// types, relationship macros, and an object store.
    Sarzak(SarzakCompilerOptions),
    /// Grace Model Compiler
    ///
    /// This is a feature-rich, general purpose model compiler that generates
    /// Rust code.
    Grace(GraceCompilerOptions),
}
