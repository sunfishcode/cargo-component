use crate::support::*;
use anyhow::{Context, Result};
use assert_cmd::prelude::*;
use cargo_component::registry::LOCK_FILE_NAME;
use predicates::str::contains;
use std::fs;
use toml_edit::{value, Document, InlineTable, Value};

mod support;

#[test]
fn help() {
    for arg in ["help registry", "registry -h", "registry --help"] {
        cargo_component(arg)
            .assert()
            .stdout(contains(
                "Interact with a local file system component registry",
            ))
            .success();
    }
}

#[test]
fn it_creates_a_registry() -> Result<()> {
    let path = create_root()?.join("registry");
    cargo_component(&format!("registry new {path}", path = path.display()))
        .assert()
        .stderr(contains("Creating local component registry"))
        .success();

    assert!(path.join("local-signing.key").is_file());
    Ok(())
}

#[test]
fn it_errors_if_registry_exists() -> Result<()> {
    let root = create_root()?;
    cargo_component(&format!("registry new {root}", root = root.display()))
        .assert()
        .stderr(contains("already exists"))
        .failure();

    Ok(())
}

#[test]
fn it_publishes_a_wit_package() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(root.join("foo.wit"), "default world foo {}")?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 foo.wit",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .stderr(contains("Publishing version 1.0.0 of package `foo/bar`"))
    .success();

    Ok(())
}

#[test]
fn it_publishes_a_module() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(root.join("foo.wasm"), wat::parse_str("(module)")?)?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 foo.wasm",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .stderr(contains("Publishing version 1.0.0 of package `foo/bar`"))
    .success();

    Ok(())
}

#[test]
fn it_publishes_a_component() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(root.join("foo.wasm"), wat::parse_str("(component)")?)?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 foo.wasm",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .stderr(contains("Publishing version 1.0.0 of package `foo/bar`"))
    .success();

    Ok(())
}

#[test]
fn it_errors_on_invalid_wit() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(root.join("foo.wit"), "not-valid")?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 foo.wit",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .stderr(contains(
        "expected `default`, `world` or `interface`, found an identifier",
    ))
    .failure();

    Ok(())
}

#[test]
fn it_resolves_a_target_wit_package() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(
        root.join("foo.wit"),
        r#"default world foo {
    import foo: func() -> string
    export bar: func() -> string
}"#,
    )?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 foo.wit",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .success();

    let project = Project::with_root(
        root,
        "component",
        &format!(
            "--registry {path} --target foo/bar@1.0.0",
            path = path.display()
        ),
    )?;

    project
        .cargo_component("build")
        .assert()
        .stderr(contains("Finished dev [unoptimized + debuginfo] target(s)"))
        .success();
    validate_component(&project.debug_wasm("component"))?;

    let path = project.root().join(LOCK_FILE_NAME);
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read lock file `{path}`", path = path.display()))?
        .replace("\r\n", "\n");

    assert!(
        contents.contains("[[package]]\nid = \"foo/bar\"\n\n[[package.version]]\nrequirement = \"^1.0.0\"\nversion = \"1.0.0\"\n"),
        "missing foo/bar dependency"
    );

    Ok(())
}

#[test]
fn it_errors_on_missing_target_package() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");

    cargo_component(&format!("registry new {path}", path = path.display()))
        .current_dir(&root)
        .assert()
        .success();

    match Project::with_root(
        root,
        "component",
        &format!(
            "--registry {path} --target foo/bar@1.0.0",
            path = path.display()
        ),
    ) {
        Ok(_) => panic!("expected command to fail"),
        Err(e) => assert!(
            e.to_string()
                .contains("package `foo/bar` does not exist in local registry"),
            "unexpected error: {e}",
        ),
    }

    Ok(())
}

#[test]
fn it_resolves_a_target_wit_package_with_document() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(
        root.join("foo.wit"),
        r#"default world foo {
    import foo: func() -> string
    export bar: func() -> string
}"#,
    )?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 foo.wit",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .success();

    let project = Project::with_root(
        root,
        "component",
        &format!(
            "--registry {path} --target foo/bar@1.0.0 --world foo",
            path = path.display()
        ),
    )?;

    project
        .cargo_component("build")
        .assert()
        .stderr(contains("Finished dev [unoptimized + debuginfo] target(s)"))
        .success();
    validate_component(&project.debug_wasm("component"))?;

    let path = project.root().join(LOCK_FILE_NAME);
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read lock file `{path}`", path = path.display()))?
        .replace("\r\n", "\n");

    assert!(
        contents.contains("[[package]]\nid = \"foo/bar\"\n\n[[package.version]]\nrequirement = \"^1.0.0\"\nversion = \"1.0.0\"\n"),
        "missing foo/bar dependency"
    );

    Ok(())
}

#[test]
fn it_errors_on_invalid_document() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(
        root.join("foo.wit"),
        r#"default world foo {
    import foo: func() -> string
    export bar: func() -> string
}"#,
    )?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 foo.wit",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .success();

    match Project::with_root(
        root,
        "component",
        &format!(
            "--registry {path} --target foo/bar@1.0.0 --world bar",
            path = path.display()
        ),
    ) {
        Ok(_) => panic!("expected command to fail"),
        Err(e) => assert!(
            e.to_string()
                .contains("target package `foo/bar` does not contain a document named `bar`"),
            "unexpected error: {e}",
        ),
    }

    Ok(())
}

#[test]
fn it_errors_on_too_many_documents() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");

    let pkg_dir = root.join("pkg");
    fs::create_dir_all(&pkg_dir)?;
    fs::write(pkg_dir.join("doc1.wit"), "default world foo {}")?;
    fs::write(pkg_dir.join("doc2.wit"), "default world foo {}")?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 {pkg}",
        path = path.display(),
        pkg = pkg_dir.display(),
    ))
    .current_dir(&root)
    .assert()
    .success();

    match Project::with_root(
        root,
        "component",
        &format!(
            "--registry {path} --target foo/bar@1.0.0",
            path = path.display()
        ),
    ) {
        Ok(_) => panic!("expected command to fail"),
        Err(e) => assert!(
            e.to_string().contains("target package `foo/bar` contains multiple documents; specify the one to use with the `world` field in the manifest file"),
            "unexpected error: {e}"
        ),
    }

    Ok(())
}

#[test]
fn it_resolves_a_target_wit_package_with_world() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(
        root.join("foo.wit"),
        r#"default world bar {
    import foo: func() -> string
    export bar: func() -> string
}"#,
    )?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 foo.wit",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .success();

    let project = Project::with_root(
        root,
        "component",
        &format!(
            "--registry {path} --target foo/bar@1.0.0 --world foo.bar",
            path = path.display()
        ),
    )?;

    project
        .cargo_component("build")
        .assert()
        .stderr(contains("Finished dev [unoptimized + debuginfo] target(s)"))
        .success();
    validate_component(&project.debug_wasm("component"))?;

    let path = project.root().join(LOCK_FILE_NAME);
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read lock file `{path}`", path = path.display()))?
        .replace("\r\n", "\n");

    assert!(
        contents.contains("[[package]]\nid = \"foo/bar\"\n\n[[package.version]]\nrequirement = \"^1.0.0\"\nversion = \"1.0.0\"\n"),
        "missing foo/bar dependency"
    );

    Ok(())
}

#[test]
#[ignore = "default world decoding is not yet implemented"]
fn it_resolves_a_target_wit_package_with_default_world() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");

    let pkg_dir = root.join("pkg");
    fs::create_dir_all(&pkg_dir)?;
    fs::write(
        pkg_dir.join("doc1.wit"),
        r#"default world bar {
    import foo: func() -> string
    export bar: func() -> string
}
world bar {
}"#,
    )?;
    fs::write(pkg_dir.join("doc2.wit"), "default world foo {}")?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 {pkg}",
        path = path.display(),
        pkg = pkg_dir.display(),
    ))
    .current_dir(&root)
    .assert()
    .success();

    let project = Project::with_root(
        root,
        "component",
        &format!(
            "--registry {path} --target foo/bar@1.0.0 --world doc1",
            path = path.display()
        ),
    )?;

    project
        .cargo_component("build")
        .assert()
        .stderr(contains("Finished dev [unoptimized + debuginfo] target(s)"))
        .success();
    validate_component(&project.debug_wasm("component"))?;

    let path = project.root().join(LOCK_FILE_NAME);
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read lock file `{path}`", path = path.display()))?
        .replace("\r\n", "\n");

    assert!(
        contents.contains("[[package]]\nid = \"foo/bar\"\n\n[package.requirements.\"^1.0.0\"]\nversion = \"1.0.0\"\n"),
        "missing foo/bar dependency"
    );

    Ok(())
}

#[test]
fn it_errors_on_invalid_world() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(
        root.join("foo.wit"),
        r#"default world foo {
    import foo: func() -> string
    export bar: func() -> string
}"#,
    )?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 foo.wit",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .success();

    match Project::with_root(
        root,
        "component",
        &format!(
            "--registry {path} --target foo/bar@1.0.0 --world foo.bar",
            path = path.display()
        ),
    ) {
        Ok(_) => panic!("expected command to fail"),
        Err(e) => assert!(
            e.to_string().contains(
                "target package `foo/bar` does not contain a world named `bar` in document `foo`"
            ),
            "unexpected error: {e}",
        ),
    }

    Ok(())
}

#[test]
fn it_errors_on_too_many_worlds() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");

    let pkg_dir = root.join("pkg");
    fs::create_dir_all(&pkg_dir)?;
    fs::write(pkg_dir.join("doc1.wit"), "world foo {} world bar {}")?;
    fs::write(pkg_dir.join("doc2.wit"), "default world foo {}")?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 {pkg}",
        path = path.display(),
        pkg = pkg_dir.display(),
    ))
    .current_dir(&root)
    .assert()
    .success();

    match Project::with_root(
        root,
        "component",
        &format!(
            "--registry {path} --target foo/bar@1.0.0 --world doc1",
            path = path.display()
        ),
    ) {
        Ok(_) => panic!("expected command to fail"),
        Err(e) => assert!(
            e.to_string().contains("target document `doc1` in package `foo/bar` contains multiple worlds; specify the one to use with the `world` field in the manifest file"),
            "unexpected error: {e}"
        ),
    }

    Ok(())
}

#[test]
fn it_errors_on_missing_dependency() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(root.join("foo.wit"), "default world foo {}")?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 foo.wit",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .success();

    let project = Project::with_root(
        root,
        "component",
        &format!("--registry {path} --target foo/bar", path = path.display()),
    )?;

    let manifest_path = project.root().join("Cargo.toml");
    let mut manifest: Document = fs::read_to_string(&manifest_path)?.parse()?;
    let dependencies = &mut manifest["package"]["metadata"]["component"]["dependencies"];
    dependencies["baz"] = value("foo/baz@1.0.0");
    fs::write(manifest_path, manifest.to_string())?;

    project
        .cargo_component("build")
        .assert()
        .stderr(contains(
            "package `foo/baz` does not exist in local registry",
        ))
        .failure();

    Ok(())
}

#[test]
fn it_errors_on_missing_dependency_version() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(root.join("foo.wit"), "default world foo {}")?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 foo.wit",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .success();

    let project = Project::with_root(
        root,
        "component",
        &format!("--registry {path} --target foo/bar", path = path.display()),
    )?;

    let manifest_path = project.root().join("Cargo.toml");
    let mut manifest: Document = fs::read_to_string(&manifest_path)?.parse()?;
    let dependencies = &mut manifest["package"]["metadata"]["component"]["dependencies"];
    dependencies["bar"] = value("foo/bar@2.0.0");
    fs::write(manifest_path, manifest.to_string())?;

    project
        .cargo_component("build")
        .assert()
        .stderr(contains(
            "a version of package `foo/bar` that satisfies version requirement `^2.0.0` was not found",
        ))
        .failure();

    Ok(())
}

#[test]
fn it_resolves_a_component_dependency() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(
        root.join("foo.wit"),
        r#"default world foo {
    import foo: func() -> string
    export bar: func() -> string
}"#,
    )?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 foo.wit",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .success();

    fs::write(
        root.join("baz.wat"),
        r#"(component (import "foo" (func (result string))) (export "export" (func 0)))"#,
    )?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/baz --version 1.2.3 baz.wat",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .success();

    let project = Project::with_root(
        root,
        "component",
        &format!("--registry {path} --target foo/bar", path = path.display()),
    )?;

    let manifest_path = project.root().join("Cargo.toml");
    let mut manifest: Document = fs::read_to_string(&manifest_path)?.parse()?;
    let dependencies = &mut manifest["package"]["metadata"]["component"]["dependencies"];
    dependencies["baz"] = value("foo/baz@1.0.0");
    fs::write(manifest_path, manifest.to_string())?;

    project
        .cargo_component("build")
        .assert()
        .stderr(contains("Finished dev [unoptimized + debuginfo] target(s)"))
        .success();
    validate_component(&project.debug_wasm("component"))?;

    let path = project.root().join(LOCK_FILE_NAME);
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read lock file `{path}`", path = path.display()))?
        .replace("\r\n", "\n");

    assert!(
        contents.contains("[[package]]\nid = \"foo/bar\"\n\n[[package.version]]\nrequirement = \"^1.0.0\"\nversion = \"1.0.0\"\n"),
        "missing foo/bar dependency"
    );

    assert!(
        contents.contains("[[package]]\nid = \"foo/baz\"\n\n[[package.version]]\nrequirement = \"^1.0.0\"\nversion = \"1.2.3\"\n"),
        "missing foo/baz dependency"
    );

    Ok(())
}

#[test]
fn it_resolves_a_wit_document_dependency() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(
        root.join("foo.wit"),
        r#"interface foo { record bar {} baz: func(b: bar) -> string }"#,
    )?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 foo.wit",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .success();

    let source = r#"use bindings::{foo, Foo};
struct Component;
impl Foo for Component {
    fn bar() -> String {
        foo::baz(foo::Bar {})
    }
}
bindings::export!(Component);
"#;

    let world = r#"default world foo {
    import foo: external-package.foo.foo
    export bar: func() -> string
}"#;

    let project = Project::with_root(root, "component", "")?;
    let manifest_path = project.root().join("Cargo.toml");
    let mut manifest: Document = fs::read_to_string(&manifest_path)?.parse()?;

    let dependencies = &mut manifest["package"]["metadata"]["component"]["target"]["dependencies"];
    dependencies["external-package"] = value("foo/bar@1.0.0");

    let registries = &mut manifest["package"]["metadata"]["component"]["registries"];
    registries["default"] = value(InlineTable::from_iter([(
        "path",
        Value::from("../registry"),
    )]));

    fs::write(manifest_path, manifest.to_string())?;
    project.file("src/lib.rs", source)?;
    project.file("wit/world.wit", world)?;

    project
        .cargo_component("build")
        .assert()
        .stderr(contains("Finished dev [unoptimized + debuginfo] target(s)"))
        .success();
    validate_component(&project.debug_wasm("component"))?;

    let path = project.root().join(LOCK_FILE_NAME);
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read lock file `{path}`", path = path.display()))?
        .replace("\r\n", "\n");

    assert!(
        contents.contains("[[package]]\nid = \"foo/bar\"\n\n[[package.version]]\nrequirement = \"^1.0.0\"\nversion = \"1.0.0\"\n"),
        "missing foo/bar dependency"
    );

    Ok(())
}

#[test]
fn it_locks_to_a_specific_version() -> Result<()> {
    let root = create_root()?;
    let path = root.join("registry");
    fs::write(
        root.join("v10.wit"),
        r#"default world foo {
    import foo: func() -> string
    export bar: func() -> string
}"#,
    )?;

    fs::write(
        root.join("v11.wit"),
        r#"default world foo {
    import renamed: func() -> string
    export bar: func() -> string
}"#,
    )?;

    cargo_component(&format!(
        "registry publish --registry {path} --id foo/bar --version 1.0.0 v10.wit",
        path = path.display()
    ))
    .current_dir(&root)
    .assert()
    .success();

    let project = Project::with_root(
        root,
        "component",
        &format!("--registry {path} --target foo/bar", path = path.display()),
    )?;

    project
        .cargo_component("build")
        .assert()
        .stderr(contains("Finished dev [unoptimized + debuginfo] target(s)"))
        .success();
    validate_component(&project.debug_wasm("component"))?;

    let lock_file_path = project.root().join(LOCK_FILE_NAME);
    let orig_contents = fs::read_to_string(&lock_file_path)
        .with_context(|| {
            format!(
                "failed to read lock file `{path}`",
                path = lock_file_path.display()
            )
        })?
        .replace("\r\n", "\n");

    assert!(
        orig_contents.contains("[[package]]\nid = \"foo/bar\"\n\n[[package.version]]\nrequirement = \"^1.0.0\"\nversion = \"1.0.0\"\n"),
        "missing foo/bar dependency"
    );

    project
        .cargo_component(&format!(
            "registry publish --registry {path} --id foo/bar --version 1.1.0 ../v11.wit",
            path = path.display()
        ))
        .assert()
        .success();

    project
        .cargo_component("build")
        .assert()
        .stderr(contains("Finished dev [unoptimized + debuginfo] target(s)"))
        .success();
    validate_component(&project.debug_wasm("component"))?;

    let contents = fs::read_to_string(&lock_file_path)
        .with_context(|| {
            format!(
                "failed to read lock file `{path}`",
                path = lock_file_path.display()
            )
        })?
        .replace("\r\n", "\n");

    assert_eq!(orig_contents, contents, "expected no change to lock file");

    Ok(())
}
