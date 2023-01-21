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
use toml;
use toml_edit::{table, value, Document};
use uuid::Uuid;

use nut::codegen::{emitln, CachingContext, SarzakModel};
use nut::sarzak::mc::SarzakModelCompiler;

use sarzak_mc::SarzakCompilerOptions;

use sarzak_cli::config::Config;

const CONFIG: &str = "sarzak.toml";

const BLANK_MODEL: &str = include_str!("../models/blank.json");
const MODEL_DIR: &str = "models";

const JSON_EXT: &str = "json";

// Exit codes
const DOMAIN_EXISTS: i32 = -1;
const MODULE_DIR_MISSING: i32 = -2;
const NOTHING_TO_DO: i32 = -3;

#[derive(Debug, Parser)]
#[command(author, version, about)]
#[command(propagate_version = true)]
struct Args {
    /// Test mode
    ///
    /// Don't execute commands, but instead print what commands would be executed.
    #[clap(long, short, action=ArgAction::SetTrue)]
    test: bool,

    /// Path to package
    ///
    /// If included, `sarzak` will create a new domain in the specified
    /// location. It must exist, and must be part of a Rust package.
    #[arg(short, long)]
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
        /// the name, and create a new Rust module. which will coincide with the name of
        /// the Rust package. It will contain a blank model file the the `models`
        /// subdirectory.
        domain: String,
    },
    /// Generate code
    ///
    /// Generate domain code from the model.
    #[command(name = "gen")]
    Generate {
        /// Domain name(s)
        ///
        /// The comma separated list of domains for which code will be generated.
        /// If this argument is not included, and there are multiple domain models,
        /// then code will be generated for all models in the domain.
        #[arg(long, short, use_value_delimiter = true, value_delimiter = ',')]
        domains: Option<Vec<String>>,

        #[command(subcommand)]
        compiler: Option<Compiler>,
    },
}

#[derive(Debug, Subcommand)]
enum Compiler {
    Sarzak {
        #[command(flatten)]
        options: SarzakCompilerOptions,
    },
}

fn main() -> Result<()> {
    pretty_env_logger::init();

    let args = Args::parse();

    // I suppose command line takes precedence over config file.

    if args.test {
        println!("Running in test mode 🧪.");
    }

    match args.command {
        Command::New { domain } => execute_command_new(&domain, &args.package_dir, args.test)?,
        Command::Generate { compiler, domains } => {
            execute_command_generate(&compiler, &domains, &args.package_dir, args.test)?
        }
    }

    Ok(())
}

fn execute_command_new(domain: &str, dir: &Option<PathBuf>, test_mode: bool) -> Result<()> {
    let rust_name = domain.to_snake_case();

    // Find the package root
    //
    let package_root = find_package_dir(dir)?;

    // Update te config file
    //
    let mut config_path = package_root.clone();
    config_path.push(CONFIG);

    if !config_path.exists() {
        // Create the config file
        debug!("💥 Creating sarzak.toml.");
        let mut config = File::create(&config_path)?;
        config.write_all(b"[domains]")?;
    }

    let mut toml_string = String::new();
    File::open(&config_path)
        .context("😱 unable to open sarzak.toml")?
        .read_to_string(&mut toml_string)?;
    let mut config = toml_string.parse::<Document>()?;

    // Check to see if domain already exists
    //
    match &config["domains"].get(&rust_name) {
        Some(_) => {
            let missive = format!("😱 domain '{}' already exists in sarzak.toml!", rust_name);
            error!("{}", &missive);
            eprintln!("{}", missive);
            std::process::exit(DOMAIN_EXISTS);
        }
        None => {}
    }

    // I don't know what this is about, but I can't just do `table["sarzak"] = table();`,
    // and I can't even move the following line further down by where it's used.
    // Weird.
    let mut sarzak = table();
    sarzak["new"] = value(true);
    let mut table = table();
    table["path"] = value(format!("models/{}.{}", rust_name, JSON_EXT));
    table["module"] = value(format!("{}", rust_name));
    table["sarzak"] = sarzak;

    config["domains"][&rust_name] = table;

    // This doesn't work, for reasons beyond my ken.
    config["domains"][&rust_name]
        .as_inline_table_mut()
        .map(|t| t.fmt());

    let mut toml_file =
        File::create(&config_path).context("😱 unable to open sarzak.toml for writing")?;
    toml_file
        .write_all(config.to_string().as_bytes())
        .context("😱 unable to write sarzak.toml!")?;

    println!(
        "Creating new domain ✨{}✨ in {}❗️",
        domain,
        package_root.to_string_lossy()
    );
    println!("The module will be called ✨{}✨.", rust_name);

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
    src_dir.push(&rust_name);
    debug!("Creating module directory {:?}.", src_dir);
    if !test_mode {
        fs::create_dir(&src_dir)
            .context(format!("😱 Failed to create directory: {:?}", src_dir))?;
    }

    // Generate a "module" .rs file
    //
    debug!("Creating {}.rs. 🥳", rust_name);
    src_dir.set_file_name(&rust_name);
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
    domains: &Option<Vec<String>>,
    package_dir: &Option<PathBuf>,
    test_mode: bool,
) -> Result<()> {
    // Find the package root
    //
    let package_root = find_package_dir(package_dir)?;

    // Open the config file
    //
    let mut config_path = package_root.clone();
    config_path.push(CONFIG);

    let mut toml = String::new();
    File::open(&config_path)
        .context("😱 unable to open sarzak.toml")?
        .read_to_string(&mut toml)?;

    let config: Config = toml::from_str(&toml)?;
    debug!("Loaded config 📝 file.");

    // Ensure that we can find the models directory
    //
    let mut model_dir = package_root.clone();
    model_dir.push(MODEL_DIR);
    anyhow::ensure!(
        model_dir.exists(),
        format!("😱 Unable to find models directory: {:?}.", model_dir)
    );
    debug!("Found model ✈️  directory.");

    // Process domains passed in on the command line.
    if let Some(domains) = domains {
        for domain in domains {
            // Spaces between commas in the domain specification result in spaces
            // in our domains list. Just skip.
            if domain != "" {
                if let Some(config) = config.domains.get(domain) {
                    let model_file = get_model_path(&model_dir, domain);
                    debug!("⭐️ Found {:?}!", model_file);

                    match compiler {
                        Some(compiler) => match compiler {
                            Compiler::Sarzak { options } => {
                                invoke_model_compiler(
                                    &compiler,
                                    &package_root,
                                    &model_file,
                                    test_mode,
                                    &config.module,
                                    domain,
                                )?;
                            }
                        },
                        None => {
                            let compiler = Compiler::Sarzak {
                                options: config.sarzak.clone(),
                            };

                            invoke_model_compiler(
                                &compiler,
                                &package_root,
                                &model_file,
                                test_mode,
                                &config.module,
                                domain,
                            )?;
                        }
                    }
                } else {
                    // Why don't I just format one string and use it twice? Why write about it
                    // and not just do it? I'm feeling insolent. 🖕
                    eprintln!("😱 No domain named {} found in {}!", domain, CONFIG);
                    warn!("did not find {} in {}", domain, CONFIG);
                }
            }
        }
    } else {
        if config.domains.len() == 0 {
            eprintln!("Nothing to do. Maybe specify a domain in sarzak.toml?");
            warn!("empty domains in sarzak.toml");

            std::process::exit(NOTHING_TO_DO);
        }
        // Iterate over all of the model files in the config
        for (domain, config) in &config.domains {
            let mut model_file = package_root.clone();
            model_file.push(&config.path);

            let compiler = Compiler::Sarzak {
                options: config.sarzak.clone(),
            };

            invoke_model_compiler(
                &compiler,
                &package_root,
                &model_file,
                test_mode,
                &config.module,
                domain,
            )?;
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
    domain: &str,
) -> Result<()> {
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

    let mut module_path = root.clone();
    module_path.push("src");
    module_path.push(module);

    if !module_path.exists() {
        // Let the user clean this up...
        let missive = format!("😱 module directory '{}' does not exist. Cannot continue. Clean things up and try again.", module_path.display());
        error!("{}", missive);
        eprint!("{}", missive);

        std::process::exit(MODULE_DIR_MISSING);
    }

    debug!("Writing output to {}.", module_path.display());

    // We have to add a bogus directory to the path so that set_file_name doesn't
    // clobber our value.
    module_path.push("bogus");

    let package = root
        .as_path()
        .components()
        .last()
        .unwrap()
        .as_os_str()
        .to_string_lossy();

    let model = SarzakModel::load_cuckoo_model(&model_file).context("😱 reading model file")?;

    println!("Generating 🧬 code for domain ✨{}✨!", domain);
    debug!("Generating 🧬 code for domain, {}!", model_file.display());

    match compiler {
        Compiler::Sarzak { options } => {
            let compiler = sarzak_mc::ModelCompiler::new();
            compiler
                .compile(&model, &package, &module_path, Box::new(options), test_mode)
                .map_err(anyhow::Error::msg)
        }
    }

    // Ok(())
}

/// Return the path to a domain model
///
fn get_model_path<S: AsRef<str>>(model_dir: &PathBuf, domain: S) -> PathBuf {
    let mut model_file = model_dir.clone();
    // Don't forget about the pop.
    model_file.push("fubar");
    model_file.set_file_name(domain.as_ref());
    model_file.set_extension(JSON_EXT);

    model_file
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
