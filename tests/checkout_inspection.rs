use lore::{
    config::{Config, ResolvedConfig},
    context::{
        InspectionPath,
        inspection::{CodeObservation, InspectionSession, InspectionSettings},
    },
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

fn project() -> (tempfile::TempDir, ResolvedConfig) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("docs")).unwrap();
    let path = dir.path().join("lore.yml");
    fs::write(&path, serde_yaml::to_string(&Config::default()).unwrap()).unwrap();
    let config = ResolvedConfig::load(&path).unwrap();
    (dir, config)
}

fn settings() -> InspectionSettings {
    InspectionSettings {
        enabled: true,
        root: Some(".".into()),
        // Keep filesystem timing tests stable on loaded CI runners; the real
        // default remains 1000 ms and is covered by the settings contract.
        max_elapsed_ms: 10_000,
        ..InspectionSettings::default()
    }
}

fn put(config: &ResolvedConfig, path: &str, text: impl AsRef<[u8]>) {
    let path = config.base.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn discover(config: &ResolvedConfig, options: &InspectionSettings) -> InspectionSession {
    InspectionSession::discover(config, "Implement payment retries", &[], &[], options).unwrap()
}

fn verify_independently(config: &ResolvedConfig, observation: &CodeObservation) {
    let bytes = fs::read(config.base.join(&observation.path)).unwrap();
    assert_eq!(
        observation.content_hash,
        format!("sha256:{:x}", Sha256::digest(&bytes))
    );
    let contents = String::from_utf8(bytes).unwrap();
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    assert!(observation.start_line > 0 && observation.end_line >= observation.start_line);
    assert_eq!(
        observation.excerpt,
        lines[observation.start_line - 1..observation.end_line].concat()
    );
    let id = serde_json::to_vec(&serde_json::json!([
        observation.path,
        observation.start_line,
        observation.end_line,
        observation.excerpt,
        observation.content_hash,
    ]))
    .unwrap();
    assert_eq!(observation.id, format!("co_{:x}", Sha256::digest(id)));
    assert!(
        observation
            .qualification
            .contains("not proof of runtime behavior")
    );
}

#[test]
fn opt_in_discovery_and_configuration_limits_are_explicit() {
    let (_dir, config) = project();
    put(&config, "src/payment.rs", "fn payment() {}\n");
    let disabled = discover(&config, &InspectionSettings::default());
    assert_eq!(disabled.report().status, "disabled");
    assert!(disabled.report().root.is_none());
    assert_eq!(disabled.report().budget.bytes_read, 0);
    assert_eq!(disabled.report().budget.index_entries, 0);
    // Test runners may put temporary directories inside their own checkout.
    // A filesystem-root configuration has no safely discoverable ancestor,
    // irrespective of the host's checkout layout.
    let mut no_ancestor = config.clone();
    no_ancestor.base = config.base.ancestors().last().unwrap().to_owned();
    let unavailable = discover(
        &no_ancestor,
        &InspectionSettings {
            enabled: true,
            root: None,
            ..settings()
        },
    );
    assert_eq!(unavailable.report().status, "unavailable");
    assert!(unavailable.candidates().is_empty());
    assert!(!unavailable.report().warnings.is_empty());
    for field in [
        "max_files",
        "max_file_bytes",
        "max_total_bytes",
        "max_index_entries",
        "max_elapsed_ms",
        "max_excerpt_lines",
    ] {
        let mut value = serde_json::to_value(settings()).unwrap();
        value[field] = 0.into();
        let invalid: InspectionSettings = serde_json::from_value(value).unwrap();
        assert!(invalid.validate().is_err(), "accepted zero {field}");
    }
    let mut unknown = serde_json::to_value(settings()).unwrap();
    unknown["run_command"] = "echo unsafe".into();
    assert!(serde_json::from_value::<InspectionSettings>(unknown).is_err());
    assert!(
        InspectionSettings {
            root: Some(config.base.clone()),
            ..settings()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn nearest_checkout_and_explicit_configuration_relative_root_are_supported() {
    let (_dir, config) = project();
    fs::create_dir(config.base.join(".git")).unwrap();
    put(&config, "src/payment.rs", "fn payment() {}\n");
    let inferred = discover(
        &config,
        &InspectionSettings {
            root: None,
            ..settings()
        },
    );
    assert_eq!(inferred.report().root.as_deref(), config.base.to_str());
    assert!(inferred.report().index_complete);
    // Worktree .git files are only markers. Their contents are never followed.
    fs::remove_dir(config.base.join(".git")).unwrap();
    put(&config, ".git", "gitdir: /outside/should-never-be-read\n");
    assert!(
        discover(
            &config,
            &InspectionSettings {
                root: None,
                ..settings()
            }
        )
        .report()
        .index_complete
    );
    put(
        &config,
        "nested/lore.yml",
        serde_yaml::to_string(&Config::default()).unwrap(),
    );
    let nested = ResolvedConfig::load(&config.base.join("nested/lore.yml")).unwrap();
    let mut selected = discover(
        &nested,
        &InspectionSettings {
            root: Some("..".into()),
            ..settings()
        },
    );
    assert_eq!(
        selected.inspect("src/payment.rs").unwrap().path,
        "src/payment.rs"
    );
}

#[test]
fn exact_hash_bound_observations_preserve_crlf_unicode_and_line_ranges() {
    let (_dir, config) = project();
    let source = "// øre, not a normalized line ending\r\nconst MAX_RETRIES: usize = 5;\r\nfn retry_payment() { /* betaling */ }\r\n";
    put(&config, "src/payment.rs", source);
    let mut session = discover(&config, &settings());
    let observation = session.inspect("src/payment.rs").unwrap();
    assert_eq!(observation.kind, "static_source");
    assert_eq!(observation.start_line, 1);
    assert_eq!(observation.end_line, 3);
    assert_eq!(observation.excerpt, source);
    verify_independently(&config, &observation);
    assert_eq!(session.report().budget.bytes_read, source.len());
    assert_eq!(session.report().budget.files_read, 1);
}

#[test]
fn task_terms_recorded_paths_and_related_tests_select_useful_small_observations() {
    let (_dir, config) = project();
    put(&config, "src/payment.rs", "fn payment_retry() {}\n");
    put(
        &config,
        "tests/payment_test.rs",
        "#[test]\nfn keeps_payment_limit() {}\n",
    );
    put(&config, "src/unrelated.rs", "fn compress_images() {}\n");
    let mut session = discover(&config, &settings());
    session.inspect_initial();
    let report = session.report();
    assert!(
        report
            .observations
            .iter()
            .any(|o| o.path == "src/payment.rs")
    );
    assert!(
        report
            .observations
            .iter()
            .any(|o| o.path == "tests/payment_test.rs" && o.kind == "static_test")
    );
    assert!(
        !report
            .observations
            .iter()
            .any(|o| o.path == "src/unrelated.rs")
    );
    assert!(report.observations.len() <= 3);
    let hinted = InspectionSession::discover(
        &config,
        "Add a feature",
        &[InspectionPath {
            root_id: "docs".into(),
            path: "src/unrelated.rs".into(),
            basis: "mentioned_in_evidence".into(),
            evidence_id: "ev_example".into(),
            historical: false,
        }],
        &[],
        &settings(),
    )
    .unwrap();
    assert_eq!(hinted.candidates()[0].path, "src/unrelated.rs");
    let explicit = InspectionSession::discover(
        &config,
        "Add a feature",
        &[],
        &["src/unrelated.rs".into()],
        &settings(),
    )
    .unwrap();
    assert_eq!(explicit.candidates()[0].path, "src/unrelated.rs");
}

#[test]
fn path_catalog_rejects_traversal_absolute_paths_and_invented_files() {
    let (_dir, config) = project();
    put(&config, "src/payment.rs", "fn payment() {}\n");
    let mut session = discover(&config, &settings());
    for path in [
        "../payment.rs",
        "src/../../payment.rs",
        "./src/payment.rs",
        "/etc/passwd",
        "C:/Users/person/payment.rs",
        "src\\payment.rs",
        "src//payment.rs",
        "src/payment.rs\0",
        "src/missing.rs",
    ] {
        assert!(session.inspect(path).is_err(), "accepted {path:?}");
    }
    assert!(session.report().observations.is_empty());
    assert_eq!(session.report().budget.bytes_read, 0);
    // A newly created file is not retroactively allowlisted in this session.
    put(&config, "src/new.rs", "fn payment() {}\n");
    assert!(session.inspect("src/new.rs").is_err());
    assert!(session.inspect("src/payment.rs").is_ok());
}

#[test]
fn ignores_hidden_secrets_generated_outputs_and_configured_exclusions() {
    let (_dir, mut config) = project();
    config.config.sources.exclude.push("blocked/**".into());
    put(&config, ".gitignore", "ignored/\nsrc/ignored.rs\n");
    for path in [
        ".hidden/payment.rs",
        ".env",
        "secrets/payment.rs",
        "credentials.json",
        "src/private_key.rs",
        "target/payment.rs",
        "node_modules/payment.js",
        "dist/payment.js",
        "lore/payment.rs",
        ".lore/payment.rs",
        "blocked/payment.rs",
        "ignored/payment.rs",
        "src/ignored.rs",
        "src/payment.pem",
        "src/payment.min.js",
        "src/payment.generated.ts",
    ] {
        put(&config, path, "must not become a source observation\n");
    }
    put(&config, "src/payment.rs", "fn payment() {}\n");
    let mut session = discover(&config, &settings());
    let paths: Vec<&str> = session
        .candidates()
        .iter()
        .map(|c| c.path.as_str())
        .collect();
    assert!(paths.contains(&"src/payment.rs"));
    assert!(
        paths
            .iter()
            .all(|p| ["src/payment.rs", "lore.yml"].contains(p)),
        "{paths:?}"
    );
    session.inspect_initial();
    assert_eq!(session.report().observations.len(), 1);
    assert!(session.report().budget.ignore_files_read > 0);
}

#[test]
fn nested_gitignore_rules_and_reinclusion_are_respected() {
    let (_dir, config) = project();
    put(&config, ".gitignore", "*.rs\n!src/\n");
    put(&config, "src/.gitignore", "!payment.rs\n");
    put(&config, "src/payment.rs", "fn payment() {}\n");
    put(&config, "src/other.rs", "fn other() {}\n");
    let session = discover(&config, &settings());
    assert!(
        session
            .candidates()
            .iter()
            .any(|c| c.path == "src/payment.rs")
    );
    assert!(
        !session
            .candidates()
            .iter()
            .any(|c| c.path == "src/other.rs")
    );
}

#[test]
fn binary_generated_and_credential_content_cannot_be_observations() {
    let (_dir, config) = project();
    put(&config, "src/payment_binary.rs", b"fn payment() {}\0\n");
    put(&config, "src/payment_invalid.rs", [0xff, 0xfe, b'x']);
    put(
        &config,
        "src/payment_generated.rs",
        "// Code generated by compiler; DO NOT EDIT.\nfn payment() {}\n",
    );
    put(&config, "payment.yml", "api_key: never-export-this-value\n");
    put(
        &config,
        "payment.rs",
        "-----BEGIN PRIVATE KEY-----\nnot-a-real-key\n",
    );
    let mut session = discover(&config, &settings());
    for path in [
        "src/payment_binary.rs",
        "src/payment_invalid.rs",
        "src/payment_generated.rs",
        "payment.yml",
        "payment.rs",
    ] {
        assert!(session.inspect(path).is_err(), "accepted {path}");
    }
    assert!(session.report().observations.is_empty());
    assert_eq!(session.report().budget.files_read, 5);
}

#[test]
fn byte_file_index_and_excerpt_budgets_bound_real_reads() {
    let (_dir, config) = project();
    let source = "fn payment() {}\n";
    put(&config, "src/payment.rs", source);
    put(&config, "src/payment_two.rs", source);
    put(&config, "src/huge.rs", "x".repeat(200));
    let options = InspectionSettings {
        max_files: 1,
        max_file_bytes: 100,
        max_total_bytes: 100,
        ..settings()
    };
    let mut session = discover(&config, &options);
    assert!(!session.candidates().iter().any(|c| c.path == "src/huge.rs"));
    let initial = session.inspect("src/payment.rs").unwrap();
    assert!(session.inspect("src/payment_two.rs").is_err());
    assert_eq!(session.report().budget.files_read, 1);
    assert!(session.report().budget.exhausted.contains(&"files".into()));
    assert!(session.revalidate(&[initial]).unwrap());
    assert_eq!(session.report().budget.files_read, 1);
    assert_eq!(session.report().budget.bytes_read, source.len() * 2);
    for _ in 0..10 {
        let _ = session.inspect("src/payment.rs");
    }
    assert!(session.report().budget.bytes_read <= 100);
    assert!(session.report().budget.exhausted.contains(&"bytes".into()));
    let partial = discover(
        &config,
        &InspectionSettings {
            max_index_entries: 2,
            ..settings()
        },
    );
    assert!(!partial.report().index_complete);
    assert!(partial.report().budget.index_entries <= 2);
    let many_lines = (0..120)
        .map(|i| format!("const LINE_{i}: u32 = {i};\n"))
        .collect::<String>()
        + "fn payment_retry() {}\n";
    put(&config, "src/payment.rs", many_lines);
    let mut short = discover(
        &config,
        &InspectionSettings {
            max_excerpt_lines: 5,
            ..settings()
        },
    );
    let observation = short.inspect("src/payment.rs").unwrap();
    assert!(observation.end_line - observation.start_line < 5);
    assert!(observation.excerpt.contains("payment_retry"));
    verify_independently(&config, &observation);
}

#[test]
fn cache_revalidation_detects_same_size_changes_and_index_additions_deletions() {
    let (_dir, config) = project();
    put(&config, "src/payment.rs", "const RETRIES: u8 = 3;\n");
    let mut first = discover(&config, &settings());
    let observed = first.inspect("src/payment.rs").unwrap();
    let mut unchanged = discover(&config, &settings());
    assert_eq!(first.report().index_digest, unchanged.report().index_digest);
    assert!(
        unchanged
            .revalidate(std::slice::from_ref(&observed))
            .unwrap()
    );
    // Preserve size and modification time: cache safety must use file bytes.
    let modified = fs::metadata(config.base.join("src/payment.rs"))
        .unwrap()
        .modified()
        .unwrap();
    put(&config, "src/payment.rs", "const RETRIES: u8 = 5;\n");
    fs::File::options()
        .write(true)
        .open(config.base.join("src/payment.rs"))
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert!(
        !unchanged
            .revalidate(std::slice::from_ref(&observed))
            .unwrap()
    );
    let after_change = discover(&config, &settings()).report().index_digest;
    put(&config, "tests/payment.rs", "fn verifies_payment() {}\n");
    let after_addition = discover(&config, &settings()).report().index_digest;
    assert_ne!(after_change, after_addition);
    fs::remove_file(config.base.join("tests/payment.rs")).unwrap();
    assert_ne!(
        after_addition,
        discover(&config, &settings()).report().index_digest
    );
    let mut corrupted = observed;
    corrupted.excerpt = "invented source".into();
    assert!(
        !discover(&config, &settings())
            .revalidate(&[corrupted])
            .unwrap()
    );
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(snapshot(&path));
        } else {
            files.insert(
                path.clone(),
                (
                    fs::read(&path).unwrap(),
                    fs::metadata(path).unwrap().modified().unwrap(),
                ),
            );
        }
    }
    files
}

#[test]
fn source_files_and_commands_are_never_executed_or_modified() {
    let (_dir, config) = project();
    put(
        &config,
        "src/payment.sh",
        "#!/bin/sh\ntouch NEVER_CREATE_THIS\nexit 19\n",
    );
    put(
        &config,
        "tests/payment.rs",
        "#[test]\nfn payment() { panic!(\"must not run\"); }\n",
    );
    let before = snapshot(&config.base);
    let mut session = discover(&config, &settings());
    session.inspect_initial();
    assert_eq!(session.report().observations.len(), 2);
    assert_eq!(before, snapshot(&config.base));
    assert!(!config.base.join("NEVER_CREATE_THIS").exists());
}

#[cfg(unix)]
#[test]
fn symlink_files_directories_roots_and_post_discovery_swaps_are_rejected() {
    use std::os::unix::fs::symlink;
    let (_dir, config) = project();
    let external = tempfile::tempdir().unwrap();
    fs::write(
        external.path().join("payment.rs"),
        "outside checkout content\n",
    )
    .unwrap();
    symlink(external.path(), config.base.join("outside")).unwrap();
    symlink(
        external.path().join("payment.rs"),
        config.base.join("payment.rs"),
    )
    .unwrap();
    put(&config, "src/payment.rs", "fn payment() {}\n");
    let mut session = discover(&config, &settings());
    assert!(
        !session
            .candidates()
            .iter()
            .any(|c| c.path == "payment.rs" || c.path.starts_with("outside/"))
    );
    assert!(
        InspectionSession::discover(
            &config,
            "payment",
            &[],
            &[],
            &InspectionSettings {
                root: Some("outside".into()),
                ..settings()
            }
        )
        .is_err()
    );
    // A catalog is not permission to follow a path replaced with a symlink.
    fs::rename(config.base.join("src"), config.base.join("original-src")).unwrap();
    symlink(external.path(), config.base.join("src")).unwrap();
    assert!(session.inspect("src/payment.rs").is_err());
    assert!(session.report().observations.is_empty());
    assert_eq!(session.report().budget.bytes_read, 0);
}

#[cfg(unix)]
#[test]
fn symlinked_ignore_files_fail_closed_without_reading_external_rules() {
    use std::os::unix::fs::symlink;
    let (_dir, config) = project();
    let external = tempfile::tempdir().unwrap();
    fs::write(external.path().join("rules"), "src/\n").unwrap();
    symlink(
        external.path().join("rules"),
        config.base.join(".gitignore"),
    )
    .unwrap();
    put(&config, "src/payment.rs", "fn payment() {}\n");
    let session = discover(&config, &settings());
    assert!(!session.report().index_complete);
    assert!(session.candidates().is_empty());
    assert_eq!(session.report().budget.bytes_read, 0);
}
