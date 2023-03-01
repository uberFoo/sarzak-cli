use std::{
    ffi::OsString,
    fs,
    fs::File,
    io::{Read, Write},
    os::unix::ffi::OsStringExt,
    path::PathBuf,
    process,
};

use anyhow::{Context, Result};
use clap::{ArgAction, Parser, Subcommand};
use heck::{ToSnakeCase, ToTitleCase};
use log::{debug, error, warn};
use pretty_env_logger;
use toml::{Table, Value};
use uuid::Uuid;

use nut::codegen::{emitln, CachingContext};
use sarzak::{domain::DomainBuilder, mc::SarzakModelCompiler};

use grace::GraceCompilerOptions;

use sarzak_cli::config::{Compiler as CompilerOptions, Config, ModuleConfig};

const SARZAK_CONFIG_TOML: &str = "sarzak.toml";

const BLANK_MODEL: &str = include_str!("../models/blank.json");
const MODEL_DIR: &str = "models";

const JSON_EXT: &str = "json";

// Exit codes
const MODULE_EXISTS: i32 = -1;
const MODULE_DIR_MISSING: i32 = -2;
const NOTHING_TO_DO: i32 = -3;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    /// Test mode
    ///
    /// Don't execute commands, but instead print what commands would be executed.
    #[clap(long, short, action=ArgAction::SetTrue)]
    test: bool,

    /// Sarzak config file
    ///
    /// The name of the config file you'd like to use, defaults to sarzak.toml.
    #[arg(long, short)]
    config: Option<PathBuf>,

    /// Path to package
    ///
    /// If included, `sarzak` will create a new domain in the specified
    /// location. It must exist, and must be part of a Rust package.
    #[arg(long, short)]
    package_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a new domain
    ///
    /// This involves creating a new module within a Rust package, as well as a
    /// new model file.
    ///
    /// This command needs to be run within a rust package, i.e., someplace
    /// below a `Cargo.toml`. Hopefully we're smart enough to sort out the
    /// details.
    New {
        /// Domain name
        ///
        /// The name of your new domain! Name it anything you like, although I
        /// haven't yet tried unicode... 🤔 One way or another we'll sort out
        /// the name, and create a new, blank model file the the `models`
        /// subdirectory.
        domain: String,
        /// Module Name
        ///
        /// The name of the Rust module that will contain the generated source
        /// code. If not supplied the module name will match the domain name.
        module: Option<String>,
    },
    /// Generate code
    ///
    /// Generate domain code from the model.
    #[command(name = "gen")]
    Generate {
        /// Module name(s)
        ///
        /// The comma separated list of modules for which code will be generated.
        /// If this argument is not included, then all modules in the sarzak.toml
        /// file will generated.
        #[arg(long, short, use_value_delimiter = true, value_delimiter = ',')]
        modules: Option<Vec<String>>,

        #[command(subcommand)]
        compiler: Option<Compiler>,
    },
}

#[derive(Debug, Subcommand)]
enum Compiler {
    /// Grate Model Compiler
    ///
    /// This is a feature-rich, general purpose model compiler that generates
    /// Rust code.
    Grace {
        #[command(flatten)]
        options: GraceCompilerOptions,
    },
}

fn main() -> Result<()> {
    pretty_env_logger::init();

    let args = Args::parse();

    // I suppose command line takes precedence over config file.

    if args.test {
        println!("Running in test mode 🧪.");
    }

    if args.config.is_some() {
        unimplemented!(
            "Selecting an alternate {} file is pending.",
            SARZAK_CONFIG_TOML
        );
    }

    match args.command {
        Command::New { domain, module } => {
            execute_command_new(&domain, &module, &args.package_dir, args.test)?
        }
        Command::Generate { compiler, modules } => {
            execute_command_generate(&compiler, &modules, &args.package_dir, args.test)?
        }
    }

    Ok(())
}

fn execute_command_new(
    domain: &str,
    module: &Option<String>,
    dir: &Option<PathBuf>,
    test_mode: bool,
) -> Result<()> {
    let rust_name = domain.to_snake_case();
    let module_name = match module {
        Some(m) => m.to_snake_case(),
        None => rust_name.clone(),
    };

    // Find the package root
    //
    let package_root = find_package_dir(dir)?;

    // Update te config file
    //
    let mut config_path = package_root.clone();
    config_path.push(SARZAK_CONFIG_TOML);

    if !test_mode {
        // We create the file here because below we open it for editing, and it's
        // easier to create a file with the [domains] table.
        if !config_path.exists() {
            // Create the config file
            debug!("💥 Creating {}.", SARZAK_CONFIG_TOML);
            let mut config = File::create(&config_path)?;
            config.write_all(b"[modules]")?;
        }

        let mut toml_string = String::new();
        File::open(&config_path)
            .context(format!("😱 unable to open {}", SARZAK_CONFIG_TOML))?
            .read_to_string(&mut toml_string)?;
        let mut config = toml_string.parse::<Table>()?;
        let modules = config
            .get_mut("modules")
            .expect(
                format!(
                    "There should be a [modules] table in {}.",
                    SARZAK_CONFIG_TOML
                )
                .as_str(),
            )
            .as_table_mut()
            .unwrap();

        // Check to see if domain already exists
        //
        match &modules.get(&module_name) {
            Some(_) => {
                let missive = format!(
                    "😱 module '{}' already exists in {}!",
                    rust_name, SARZAK_CONFIG_TOML
                );
                error!("{}", &missive);
                eprintln!("{}", missive);
                std::process::exit(MODULE_EXISTS);
            }
            None => {}
        }

        let options = CompilerOptions::Grace(GraceCompilerOptions::default());
        let module_config = ModuleConfig {
            model: format!("models/{}.{}", rust_name, JSON_EXT).into(),
            compiler: options,
        };

        modules.insert(module_name.clone(), Value::try_from(module_config).unwrap());

        let mut toml_file = File::create(&config_path).context(format!(
            "😱 unable to open {} for writing",
            SARZAK_CONFIG_TOML
        ))?;
        toml_file
            .write_all(config.to_string().as_bytes())
            .context(format!("😱 unable to write {}!", SARZAK_CONFIG_TOML))?;
    }

    println!(
        "Creating new domain ✨{}✨ in {}❗️",
        domain,
        package_root.to_string_lossy()
    );
    println!("The module will be called ✨{}✨.", module_name);

    // Write a blank model file.
    //
    let mut model_file = package_root.clone();
    model_file.push(MODEL_DIR);

    // Make sure the directory exists.
    //
    fs::create_dir_all(&model_file).context(format!("😱 Failed to create models directory."))?;

    // Interesting aside. PathBuf::set_file_name does a pop first.
    model_file.push("fubar");

    model_file.set_file_name(&rust_name);
    model_file.set_extension(JSON_EXT);

    debug!("Creating blank model 🐶 file at {:?}.", model_file);
    if !test_mode {
        let model = BLANK_MODEL.replace("Paper::blank", &domain);
        File::create(&model_file)
            .context(format!("😱 Failed to create file: {:?}", model_file))?
            .write_all(model.as_bytes())
            .context(format!("😱 Failed to write to file: {:?}", model_file))?;
    }

    // Create a new directory for the module
    //
    let mut src_dir = package_root.clone();
    src_dir.push("src");
    src_dir.push(&module_name);
    debug!("Creating module directory {:?}.", src_dir);
    if !test_mode {
        fs::create_dir(&src_dir)
            .context(format!("😱 Failed to create directory: {:?}", src_dir))?;
    }

    // Generate a "module" .rs file
    //
    debug!("Creating {}.rs. 🥳", module_name);
    src_dir.set_file_name(&module_name);
    src_dir.set_extension("rs");

    if !test_mode {
        let contents = generate_module_file(&domain);
        File::create(&src_dir)
            .context(format!("😱 Failed to create file: {:?}", src_dir))?
            .write_all(contents.as_bytes())
            .context(format!("😱 Failed to write to file: {:?}", src_dir))?;
    }

    // Update `lib.rs` with the new module.
    //
    // I wonder is there's a way to parse the file as rust code, edit
    // the tokenstream, and then write it back out? Nicely formatted?
    //
    // Thinking that this waits. There are issues to overcome. The first
    // is that we can't include the new module because it has no source
    // files. We can't generate source files until we have a model.
    // At least that's how the code gen code works now. They all fail
    // (panic) trying to read objects. In any case, code gen should
    // happen first.

    Ok(())
}

/// Generate a <domain>.rs file
///
/// I guess this would have made a good template.
///
/// This needs to be moved to the compiler. It should be responsible for creating
/// _all_ source code in it's module.
fn generate_module_file(domain: &str) -> String {
    let mut context = CachingContext::new();

    let uuid = Uuid::new_v5(&Uuid::NAMESPACE_OID, domain.as_bytes());

    emitln!(context, "//! {} Domain", domain.to_title_case());
    emitln!(context, "//!");
    emitln!(
        context,
        "//! This file was generated by: `sarzak new \"{}\"`.",
        domain
    );
    emitln!(context, "use uuid::{uuid, Uuid};");
    emitln!(context, "");
    emitln!(context, "pub mod macros;");
    emitln!(context, "pub mod store;");
    emitln!(context, "pub mod types;");
    emitln!(context, "");
    emitln!(context, "pub use store::ObjectStore;");
    emitln!(context, "pub use types::*;");
    emitln!(context, "pub use macros::*;");
    emitln!(context, "");
    emitln!(context, "// {}", domain);
    emitln!(context, "pub const UUID_NS: Uuid = uuid!(\"{}\");", uuid);
    emitln!(context, "");
    emitln!(context, "#[cfg(test)]");
    emitln!(context, "mod tests {");
    context.increase_indent();
    emitln!(context, "use super::*;");
    emitln!(context, "");
    emitln!(context, "#[test]");
    emitln!(context, "fn test() {");
    emitln!(context, "}");
    context.decrease_indent();
    emitln!(context, "}");

    context.commit()
}

fn execute_command_generate(
    compiler: &Option<Compiler>,
    modules: &Option<Vec<String>>,
    package_dir: &Option<PathBuf>,
    test_mode: bool,
) -> Result<()> {
    // Find the package root
    //
    let package_root = find_package_dir(package_dir)?;

    // Open the config file
    //
    let mut config_path = package_root.clone();
    config_path.push(SARZAK_CONFIG_TOML);

    let mut toml = String::new();
    File::open(&config_path)
        .context(format!("😱 unable to open {}", SARZAK_CONFIG_TOML))?
        .read_to_string(&mut toml)?;

    let config: Config = toml::from_str(&toml)?;
    debug!("Loaded config 📝 file.");

    // Process modules passed in on the command line.
    if let Some(modules) = modules {
        // Ensure that we can find the models directory
        //
        let mut model_dir = package_root.clone();
        model_dir.push(MODEL_DIR);
        anyhow::ensure!(
            model_dir.exists(),
            format!(
                "😱 Unable to find models directory: {}.",
                model_dir.display()
            )
        );
        debug!("Found model ✈️  directory.");

        for module in modules {
            // Spaces between commas in the module specification result in spaces
            // in our domains list. Just skip.
            // Last time I put spaces in the list, the parser failed. So this is wonky.
            if module != "" {
                if let Some(module_config) = config.modules.get(module) {
                    let mut model_file = module_config.model.clone();
                    if !model_file.exists() {
                        model_file = model_dir.clone();
                        model_file.push(&module_config.model);
                        anyhow::ensure!(
                            model_file.exists(),
                            format!("😱 unable to load model {}", model_file.display())
                        );
                    }
                    debug!("⭐️ Found {:?}!", model_file);

                    // We are matching on the compiler that may have been sent
                    // as a parameter. If it is_some() then it was passed in
                    // on the command line. If it's None, we read the value
                    // from sarzak.toml.
                    match compiler {
                        Some(compiler) => match compiler {
                            Compiler::Grace { options: _ } => {
                                invoke_model_compiler(
                                    &compiler,
                                    &package_root,
                                    &model_file,
                                    test_mode,
                                    &module,
                                )?;
                            }
                        },
                        None => {
                            let compiler = match &module_config.compiler {
                                CompilerOptions::Grace(options) => Compiler::Grace {
                                    options: options.clone(),
                                },
                            };

                            invoke_model_compiler(
                                &compiler,
                                &package_root,
                                &model_file,
                                test_mode,
                                &module,
                            )?;
                        }
                    }
                } else {
                    // Why don't I just format one string and use it twice? Why write about it
                    // and not just do it? I'm feeling insolent. 🖕
                    eprintln!(
                        "😱 No module named {} found in {}!",
                        module, SARZAK_CONFIG_TOML
                    );
                    warn!("did not find {} in {}", module, SARZAK_CONFIG_TOML);
                }
            }
        }
    } else {
        // No modules were passed in via the command line. Use the sarzak.toml
        // file for modules.

        if config.modules.len() == 0 {
            eprintln!(
                "Nothing to do. Maybe specify a domain in {}?",
                SARZAK_CONFIG_TOML
            );
            warn!("empty domains in {}", SARZAK_CONFIG_TOML);

            std::process::exit(NOTHING_TO_DO);
        }
        // Iterate over all of the modules files in the config
        for (module, config) in &config.modules {
            let mut model_file = package_root.clone();
            model_file.push(&config.model);

            let compiler = match &config.compiler {
                CompilerOptions::Grace(options) => Compiler::Grace {
                    options: options.clone(),
                },
            };

            invoke_model_compiler(&compiler, &package_root, &model_file, test_mode, &module)?;
        }
    }

    Ok(())
}

fn invoke_model_compiler(
    compiler: &Compiler,
    root: &PathBuf,
    model_file: &PathBuf,
    test_mode: bool,
    module: &str,
) -> Result<()> {
    log::debug!(
        "invoking model compiler `{:?}` on model `{}` for module `{}`",
        compiler,
        model_file.display(),
        module
    );
    // Check that the path exists, and that it's a file. From there we just
    // have to trust...
    anyhow::ensure!(
        model_file.exists(),
        format!("😱 Model file ({:?}) does not exist!", model_file)
    );
    anyhow::ensure!(
        model_file.is_file(),
        format!("😱 {:?} is not a model file!", model_file)
    );
    if let Some(extension) = model_file.extension() {
        anyhow::ensure!(
            extension == JSON_EXT,
            format!("😱 {:?} is not a json file!", model_file)
        );
    } else {
        anyhow::bail!(format!("😱 {:?} is not a json file!", model_file));
    }

    let mut src_path = root.clone();
    src_path.push("src");

    // We have to add a bogus directory to the path so that set_file_name doesn't
    // clobber our value.
    // module_path.push("bogus");

    let _package = root
        .as_path()
        .components()
        .last()
        .unwrap()
        .as_os_str()
        .to_string_lossy();

    let model = DomainBuilder::new()
        .cuckoo_model(&model_file)
        .context("😱 reading model file")?;

    println!(
        "Generating 🧬 code for module `{}` from domain ✨{}✨!",
        module,
        model_file.file_stem().unwrap().to_str().unwrap()
    );
    debug!("Generating 🧬 code for domain, {}!", model_file.display());

    match compiler {
        Compiler::Grace { options } => {
            let compiler = grace::ModelCompiler::default();
            compiler
                .compile(
                    model,
                    &root.file_stem().unwrap().to_str().unwrap(),
                    &module,
                    &src_path,
                    Box::new(options),
                    test_mode,
                )
                .map_err(anyhow::Error::msg)
        }
    }
}

fn find_package_dir(start_dir: &Option<PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = start_dir {
        std::env::set_current_dir(&dir)?;
    }

    // Figure out where Cargo.toml is located.
    //
    let output = process::Command::new("cargo")
        .arg("locate-project")
        .arg("--message-format")
        .arg("plain")
        .output()
        .context(
            "😱 Tried running `cargo locate-project to no avail. \
                Maybe you need to add cargo to you path?",
        )?;

    anyhow::ensure!(
        output.status.success(),
        format!(
            "😱 Unable to find package in directory: {:?}.",
            std::env::current_dir()?
        )
    );

    let mut stdout = output.stdout;

    // I don't know if it's kosher, but this does nicely to get rid of
    // that newline character.
    stdout.pop();
    let os_string = OsString::from_vec(stdout);
    let mut package_root = PathBuf::from(os_string);
    // Get rid of Cargo.toml
    package_root.pop();

    debug!("Found root 🦀 at {:?}!", package_root);

    Ok(package_root)
}
