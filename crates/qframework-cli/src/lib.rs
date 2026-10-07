use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use heck::{ToSnakeCase, ToUpperCamelCase};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug)]
enum Kind {
    Command,
    Query,
    System,
    Model,
    Utility,
    Controller,
    Event,
    App,
}

impl Kind {
    fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "command" | "commands" => Some(Self::Command),
            "query" | "queries" => Some(Self::Query),
            "system" | "systems" => Some(Self::System),
            "model" | "models" => Some(Self::Model),
            "utility" | "utilities" => Some(Self::Utility),
            "controller" | "controllers" => Some(Self::Controller),
            "event" | "events" => Some(Self::Event),
            "app" | "application" => Some(Self::App),
            _ => None,
        }
    }

    fn directory(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Query => "query",
            Self::System => "system",
            Self::Model => "model",
            Self::Utility => "utility",
            Self::Controller => "controller",
            Self::Event => "event",
            Self::App => "app",
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::Command => "Command",
            Self::Query => "Query",
            Self::System => "System",
            Self::Model => "Model",
            Self::Utility => "Utility",
            Self::Controller => "Controller",
            Self::Event => "Event",
            Self::App => "App",
        }
    }
}

#[derive(Debug)]
struct Options {
    kind: Kind,
    name: String,
    path: PathBuf,
    force: bool,
    no_mod: bool,
    godot: bool,
    architecture: Option<String>,
}

pub fn run_cli() {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => {}
        Err(Error::Help) => print_help(),
        Err(Error::Version) => println!("qframework {VERSION}"),
        Err(error) => {
            eprintln!("error: {error}");
            eprintln!("Run `qframework help` for usage.");
            std::process::exit(2);
        }
    }
}

fn run(args: &[String]) -> Result<(), Error> {
    if args.is_empty()
        || args[0] == "help"
        || args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        return Err(Error::Help);
    }
    if matches!(args[0].as_str(), "version" | "--version" | "-V") {
        return Err(Error::Version);
    }

    let mut index = 0;
    if matches!(args[0].as_str(), "new" | "generate" | "g") {
        index += 1;
    }
    let kind = args
        .get(index)
        .and_then(|value| Kind::parse(value))
        .ok_or_else(|| Error::Usage("expected a kind: command, query, system, model, utility, controller, event, or app".into()))?;
    index += 1;
    let name = args
        .get(index)
        .ok_or_else(|| Error::Usage("expected a name, such as PlayerAttack".into()))?
        .clone();
    index += 1;

    let mut path = PathBuf::from(".");
    let mut force = false;
    let mut no_mod = false;
    let mut godot = false;
    let mut architecture = None;
    while let Some(arg) = args.get(index) {
        match arg.as_str() {
            "--force" | "-f" => force = true,
            "--no-mod" => no_mod = true,
            "--godot" => godot = true,
            "--architecture" => {
                index += 1;
                architecture = Some(
                    args.get(index)
                        .ok_or_else(|| {
                            Error::Usage("--architecture requires a Rust type path".into())
                        })?
                        .clone(),
                );
            }
            "--path" | "-p" => {
                index += 1;
                let value = args
                    .get(index)
                    .filter(|value| !value.is_empty() && !value.starts_with('-'))
                    .ok_or_else(|| Error::Usage("--path requires a directory".into()))?;
                path = PathBuf::from(value);
            }
            value if value.starts_with("--path=") => {
                path = PathBuf::from(value.trim_start_matches("--path="));
                if path.as_os_str().is_empty() {
                    return Err(Error::Usage("--path requires a directory".into()));
                }
            }
            value if value.starts_with('-') => {
                return Err(Error::Usage(format!("unknown option `{value}`")));
            }
            value => return Err(Error::Usage(format!("unexpected argument `{value}`"))),
        }
        index += 1;
    }

    if (godot || architecture.is_some()) && !matches!(kind, Kind::Controller) {
        return Err(Error::Usage(
            "--godot and --architecture are only supported for controller templates".into(),
        ));
    }
    if let Some(value) = &architecture {
        syn::parse_str::<syn::Path>(value).map_err(|_| {
            Error::Usage(
                "--architecture requires a Rust type path, such as crate::app::GameApp".into(),
            )
        })?;
    }
    generate(Options {
        kind,
        name,
        path,
        force,
        no_mod,
        godot,
        architecture,
    })
}

fn generate(options: Options) -> Result<(), Error> {
    let (type_name, file_stem) = normalize_name(&options.name, options.kind)?;
    let src = source_directory(&options.path);
    let module_dir = src.join(options.kind.directory());
    let file = module_dir.join(format!("{file_stem}.rs"));
    if file.exists() && !options.force {
        return Err(Error::AlreadyExists(file));
    }

    // Validate the module file before creating the template to avoid partial output on conflicts.
    let module_update = if options.no_mod {
        None
    } else {
        prepare_module(&src, options.kind.directory(), &file_stem)?
    };
    fs::create_dir_all(&module_dir).map_err(Error::io)?;
    let content = if matches!(options.kind, Kind::Controller) {
        controller_template(&type_name, options.godot, options.architecture.as_deref())
    } else {
        template(options.kind, &type_name)
    };
    write_file(&file, content.as_bytes(), options.force)?;
    if let Some((path, content)) = module_update {
        fs::write(&path, content).map_err(Error::io)?;
        println!("updated {}", display_path(&path));
    }

    println!("created {}", display_path(&file));
    Ok(())
}

fn source_directory(path: &Path) -> PathBuf {
    if path.file_name().is_some_and(|name| name == "src") {
        path.to_path_buf()
    } else {
        path.join("src")
    }
}

fn normalize_name(input: &str, kind: Kind) -> Result<(String, String), Error> {
    if input.is_empty()
        || !input
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ' '))
    {
        return Err(Error::Usage(
            "name must contain ASCII letters, digits, underscores, hyphens, or spaces".into(),
        ));
    }
    let mut pascal = input.to_upper_camel_case();
    if pascal.is_empty() || syn::parse_str::<syn::Ident>(&pascal).is_err() {
        return Err(Error::Usage(format!(
            "`{input}` is not a valid Rust type name"
        )));
    }
    let suffix = kind.suffix();
    if !pascal.ends_with(suffix) {
        pascal.push_str(suffix);
    }
    let stem = pascal.to_snake_case();
    Ok((pascal, stem))
}

fn template(kind: Kind, type_name: &str) -> String {
    match kind {
        Kind::Command => format!(
            "use qframework_core::prelude::*;\n\npub struct {type_name};\n\nimpl ICommand for {type_name} {{\n    type Output = ();\n\n    fn execute(&self, _ctx: &CommandContext) -> Self::Output {{\n        todo!(\"Implement {type_name}\");\n    }}\n}}\n"
        ),
        Kind::Query => format!(
            "use qframework_core::prelude::*;\n\npub struct {type_name};\n\nimpl IQuery for {type_name} {{\n    type Result = ();\n\n    fn do_query(&self, _ctx: &QueryContext) -> Self::Result {{\n        todo!(\"Implement {type_name}\");\n    }}\n}}\n"
        ),
        Kind::System => format!(
            "use qframework_core::prelude::*;\n\n#[derive(Default, ISystem)]\n#[system(init = Self::on_init, deinit = Self::on_deinit)]\npub struct {type_name} {{\n    arch: ArchRef,\n}}\n\nimpl {type_name} {{\n    fn on_init(&self) {{\n        // Initialize business logic and subscriptions here.\n    }}\n\n    fn on_deinit(&self) {{\n        // Release resources and subscriptions here.\n    }}\n}}\n"
        ),
        Kind::Model => format!(
            "use qframework_core::prelude::*;\n\n#[derive(Default, IModel)]\npub struct {type_name} {{\n    arch: ArchRef,\n}}\n"
        ),
        Kind::Utility => format!(
            "use qframework_core::prelude::*;\n\n#[derive(Default, IUtility)]\npub struct {type_name};\n"
        ),
        Kind::Controller => controller_template(type_name, false, None),
        Kind::Event => format!("#[derive(Debug, Clone)]\npub struct {type_name};\n"),
        Kind::App => format!(
            "use qframework_core::prelude::*;\n\npub struct {type_name};\n\nimpl QApplication for {type_name} {{\n    fn build() -> ArchitectureBuilder {{\n        ArchitectureBuilder::new()\n        // Register models, systems, and utilities here.\n    }}\n}}\n"
        ),
    }
}

fn controller_template(type_name: &str, godot: bool, architecture: Option<&str>) -> String {
    let attribute = architecture
        .map(|path| format!("#[controller(architecture = {path})]\n"))
        .unwrap_or_default();
    if godot {
        format!(
            "use godot::classes::{{INode, Node}};\nuse godot::prelude::*;\nuse qframework_godot::prelude::*;\n\n#[derive(GodotClass, IController)]\n#[class(base = Node)]\n{attribute}pub struct {type_name} {{\n    arch: ArchRef,\n    base: Base<Node>,\n}}\n\n#[godot_api]\nimpl INode for {type_name} {{\n    fn init(base: Base<Node>) -> Self {{\n        Self {{ arch: ArchRef::new(), base }}\n    }}\n\n    fn ready(&mut self) {{\n        // Subscribe to events and properties here.\n    }}\n}}\n"
        )
    } else {
        format!(
            "use qframework_core::prelude::*;\n\n#[derive(Default, IController)]\n{attribute}pub struct {type_name} {{\n    arch: ArchRef,\n}}\n"
        )
    }
}

fn write_file(path: &Path, content: &[u8], force: bool) -> Result<(), Error> {
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    if force {
        options.truncate(true);
    } else {
        options.create_new(true);
    }
    let mut file = options.open(path).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            Error::AlreadyExists(path.to_path_buf())
        } else {
            Error::io(error)
        }
    })?;
    file.write_all(content).map_err(Error::io)
}

fn prepare_module(
    src: &Path,
    directory: &str,
    stem: &str,
) -> Result<Option<(PathBuf, String)>, Error> {
    let sibling = src.join(format!("{directory}.rs"));
    let nested = src.join(directory).join("mod.rs");
    if sibling.exists() && nested.exists() {
        return Err(Error::Usage(format!(
            "both {} and {} exist; resolve the module conflict or use --no-mod",
            display_path(&sibling),
            display_path(&nested)
        )));
    }
    let path = if nested.exists() { nested } else { sibling };
    let declaration = format!("pub mod {stem};");
    let mut content = if path.exists() {
        fs::read_to_string(&path).map_err(Error::io)?
    } else {
        String::new()
    };
    let parsed = syn::parse_file(&content)
        .map_err(|error| Error::Usage(format!("cannot parse {}: {error}", display_path(&path))))?;
    for item in parsed.items {
        if let syn::Item::Mod(module) = item
            && module.ident == stem
        {
            if module.content.is_some() || !module.attrs.is_empty() {
                return Err(Error::Usage(format!(
                    "module `{stem}` already has a body or attributes in {}; use --no-mod to manage it manually",
                    display_path(&path)
                )));
            }
            return Ok(None);
        }
    }
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    if !content.is_empty() && !content.ends_with('\n') {
        content.push_str(newline);
    }
    content.push_str(&declaration);
    content.push_str(newline);
    Ok(Some((path, content)))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn print_help() {
    println!("QFramework Rust scaffolding generator {VERSION}");
    println!();
    println!("Usage: qframework <new|generate> <kind> <name> [options]");
    println!("       qframework <kind> <name> [options]");
    println!("       qf g <kind> <name> [options]");
    println!();
    println!("Kinds: command, query, system, model, utility, controller, event, app");
    println!("Options:");
    println!("  -p, --path <dir>  project root or src directory (default: current directory)");
    println!("  -f, --force       overwrite the generated source file");
    println!("      --no-mod      do not create or update the parent module file");
    println!("      --godot       generate a Godot Node controller");
    println!("      --architecture <path>  bind a controller to a QApplication type");
    println!("  -h, --help        show this help");
    println!("  -V, --version     show version");
}

#[derive(Debug)]
enum Error {
    Help,
    Version,
    Usage(String),
    AlreadyExists(PathBuf),
    Io(io::Error),
}

impl Error {
    fn io(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Help => write!(formatter, "help requested"),
            Self::Version => write!(formatter, "version requested"),
            Self::Usage(message) => formatter.write_str(message),
            Self::AlreadyExists(path) => write!(
                formatter,
                "{} already exists; use --force to overwrite",
                display_path(path)
            ),
            Self::Io(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_normalized_with_kind_suffix() {
        assert_eq!(
            normalize_name("player_attack", Kind::Command).unwrap(),
            ("PlayerAttackCommand".into(), "player_attack_command".into())
        );
        assert_eq!(
            normalize_name("HTTPServer", Kind::System).unwrap().0,
            "HttpServerSystem"
        );
        assert_eq!(
            normalize_name("PlayerModel", Kind::Model).unwrap().0,
            "PlayerModel"
        );
    }
}
