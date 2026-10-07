use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/qframework-cli-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_qframework"))
            .args(args)
            .arg("--path")
            .arg(&self.0)
            .output()
            .unwrap()
    }

    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.0.join(path)).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn generation_preserves_existing_source_and_deduplicates_modules() {
    let fixture = Fixture::new();
    success(fixture.run(&["new", "command", "player_attack"]));
    let path = fixture.0.join("src/command/player_attack_command.rs");
    fs::write(&path, "// user edits\n").unwrap();

    let output = fixture.run(&["g", "command", "PlayerAttackCommand"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "// user edits\n");

    success(fixture.run(&["generate", "command", "PlayerAttack", "--force"]));
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains("impl ICommand for PlayerAttackCommand")
    );
    assert_eq!(
        fixture.read("src/command.rs"),
        "pub mod player_attack_command;\n"
    );
}

#[test]
fn existing_mod_rs_and_private_declarations_are_preserved() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.0.join("src/model")).unwrap();
    fs::write(
        fixture.0.join("src/model/mod.rs"),
        "// existing module\r\nmod player_model; // custom visibility\r\n",
    )
    .unwrap();
    success(fixture.run(&["model", "Player"]));
    assert!(!fixture.0.join("src/model.rs").exists());
    assert_eq!(
        fixture.read("src/model/mod.rs"),
        "// existing module\r\nmod player_model; // custom visibility\r\n"
    );
    success(fixture.run(&["model", "Inventory"]));
    assert!(
        fixture
            .read("src/model/mod.rs")
            .ends_with("pub mod inventory_model;\r\n")
    );
}

#[test]
fn invalid_arguments_and_module_conflicts_do_not_create_templates() {
    let fixture = Fixture::new();
    for name in [
        "",
        "../outside",
        "123player",
        "_",
        "type; injected",
        "\u{4e2d}\u{6587}",
    ] {
        assert!(!fixture.run(&["model", name]).status.success());
    }
    assert!(!fixture.0.join("src").exists());
    assert!(
        !fixture
            .run(&["model", "Player", "--godot"])
            .status
            .success()
    );
    assert!(
        !fixture
            .run(&["controller", "Player", "--architecture", "bad;path"])
            .status
            .success()
    );
    assert!(
        !fixture
            .run(&["command", "Attack", "--unknown"])
            .status
            .success()
    );
    assert!(
        !fixture
            .run(&["command", "Attack", "--path"])
            .status
            .success()
    );

    fs::create_dir_all(fixture.0.join("src/model")).unwrap();
    fs::write(fixture.0.join("src/model.rs"), "pub mod player_model {}\n").unwrap();
    assert!(!fixture.run(&["model", "Player"]).status.success());
    assert!(!fixture.0.join("src/model/player_model.rs").exists());
    fs::write(fixture.0.join("src/model/mod.rs"), "").unwrap();
    assert!(!fixture.run(&["model", "Inventory"]).status.success());
    assert!(!fixture.0.join("src/model/inventory_model.rs").exists());
    success(fixture.run(&["model", "Player", "--no-mod"]));
    assert_eq!(fixture.read("src/model.rs"), "pub mod player_model {}\n");
}

#[test]
fn src_path_and_godot_controller_are_supported() {
    let fixture = Fixture::new();
    let output = Command::new(env!("CARGO_BIN_EXE_qframework"))
        .args([
            "controller",
            "Player",
            "--godot",
            "--architecture",
            "crate::app::GameApp",
            "--path",
        ])
        .arg(fixture.0.join("src"))
        .output()
        .unwrap();
    success(output);
    let source = fixture.read("src/controller/player_controller.rs");
    assert!(source.contains("#[derive(GodotClass, IController)]"));
    assert!(source.contains("#[controller(architecture = crate::app::GameApp)]"));
    assert!(source.contains("impl INode for PlayerController"));
    syn::parse_file(&source).unwrap();
    assert!(!fixture.0.join("src/src").exists());
}

#[test]
fn all_core_templates_compile_together() {
    let fixture = Fixture::new();
    for (kind, name) in [
        ("command", "Attack"),
        ("query", "GetPlayer"),
        ("system", "Combat"),
        ("model", "Player"),
        ("utility", "Save"),
        ("event", "PlayerChanged"),
        ("app", "Game"),
    ] {
        success(fixture.run(&["new", kind, name]));
    }
    success(fixture.run(&[
        "controller",
        "Player",
        "--architecture",
        "crate::app::game_app::GameApp",
    ]));
    fs::write(fixture.0.join("src/lib.rs"), "pub mod command;\npub mod query;\npub mod system;\npub mod model;\npub mod utility;\npub mod event;\npub mod app;\npub mod controller;\n").unwrap();
    let core = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("qframework-core");
    let core = core.to_string_lossy().replace('\\', "/");
    fs::write(fixture.0.join("Cargo.toml"), format!("[package]\nname = \"generated-templates\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nqframework-core = {{ path = \"{core}\" }}\n")).unwrap();
    let output = Command::new(env!("CARGO"))
        .args(["check", "--offline", "--manifest-path"])
        .arg(fixture.0.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/qframework-cli-template-build"),
        )
        .output()
        .unwrap();
    success(output);
}

#[test]
fn help_and_version_exit_successfully() {
    for args in [
        vec![],
        vec!["help"],
        vec!["generate", "--help"],
        vec!["--version"],
    ] {
        success(
            Command::new(env!("CARGO_BIN_EXE_qframework"))
                .args(args)
                .output()
                .unwrap(),
        );
    }
}

#[test]
fn short_executable_and_generate_alias_match_full_command() {
    let full = Fixture::new();
    let short = Fixture::new();
    success(full.run(&["generate", "command", "PlayerAttack"]));
    success(
        Command::new(env!("CARGO_BIN_EXE_qf"))
            .args(["g", "command", "PlayerAttack", "--path"])
            .arg(&short.0)
            .output()
            .unwrap(),
    );
    for path in ["src/command.rs", "src/command/player_attack_command.rs"] {
        assert_eq!(full.read(path), short.read(path));
    }
}
