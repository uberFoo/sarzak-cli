use std::{collections::HashMap, path::PathBuf};

use serde::Deserialize;

use sarzak_mc::SarzakCompilerOptions;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub domains: HashMap<String, DomainConfig>,
}

#[derive(Debug, Deserialize)]
pub struct DomainConfig {
    pub path: PathBuf,
    pub module: String,
    /// Sarzak model compiler
    ///
    /// First off, everything in [`SarzakCompilerOptions`] is an `Option`. The way
    /// the TOML parser works since they are all optional we can specify defaults
    /// as `sarzak = {}`.
    ///
    /// Now, I'd love that this be `compiler: CompilerEnum`, but I can't get the
    /// parser to grok my meaning. Once I get another compiler, I'll have to make
    /// them each optional, which is going to fuck with things. You know, if I
    /// can do it in JSON, I don't know why I can't do it in TOML. If If can't
    /// do what I need in TOML, I could switch to JSON. That would be ugly. Maybe
    /// YAML?
    pub sarzak: SarzakCompilerOptions,
}
