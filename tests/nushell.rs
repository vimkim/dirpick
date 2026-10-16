use std::{fs, os::unix::fs::PermissionsExt, process::Command};
use tempfile::tempdir;

#[test]
fn packaged_nushell_wrapper_changes_actual_pwd_to_exact_directory_names() {
    let root = tempdir().unwrap();
    let wrapper = Command::new(env!("CARGO_BIN_EXE_dirpick"))
        .arg("--print-nu-wrapper")
        .output()
        .unwrap();
    assert!(
        wrapper.status.success(),
        "{}",
        String::from_utf8_lossy(&wrapper.stderr)
    );
    fs::write(root.path().join("dirpick.nu"), wrapper.stdout).unwrap();
    let fake = root.path().join("dirpick");
    fs::write(
        &fake,
        "#!/bin/sh\n/bin/cat \"$DIRPICK_TEST_STDOUT\"\nexit \"$DIRPICK_TEST_STATUS\"\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(root.path().join("check.nu"), "use ./dirpick.nu dp\ntry { dp } catch { |err| print --stderr $err.msg }\n$env.PWD | to json --raw\n").unwrap();
    for name in [
        "plain",
        "-leading",
        "space 'quote' \"한글\"",
        "trailing ",
        "trailing\t",
        "trailing\n",
        "embedded\nnewline",
        "tab\t \n",
    ] {
        let destination = root.path().join(name);
        fs::create_dir(&destination).unwrap();
        let record = root.path().join("result.json");
        fs::write(
            &record,
            serde_json::to_vec(&serde_json::json!({"action":"cd", "path": destination})).unwrap(),
        )
        .unwrap();
        let output = Command::new("nu")
            .args(["--no-config-file", "check.nu"])
            .current_dir(root.path())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    root.path().display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .env("DIRPICK_TEST_STDOUT", &record)
            .env("DIRPICK_TEST_STATUS", "0")
            .output()
            .expect("Nushell is required for the integration acceptance check");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let pwd: String = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            pwd,
            destination.to_str().unwrap(),
            "name={name:?}, stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn nushell_rejects_invalid_output_and_failed_or_cancelled_pickers_without_cd() {
    let root = tempdir().unwrap();
    let wrapper = Command::new(env!("CARGO_BIN_EXE_dirpick"))
        .arg("--print-nu-wrapper")
        .output()
        .unwrap();
    assert!(wrapper.status.success());
    fs::write(root.path().join("dirpick.nu"), wrapper.stdout).unwrap();
    let fake = root.path().join("dirpick");
    fs::write(
        &fake,
        "#!/bin/sh\n/bin/cat \"$DIRPICK_TEST_STDOUT\"\nexit \"$DIRPICK_TEST_STATUS\"\n",
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(root.path().join("check.nu"), "use ./dirpick.nu dp\ntry { dp } catch { |err| print --stderr $err.msg }\n$env.PWD | to json --raw\n").unwrap();
    let file = root.path().join("regular-file");
    fs::write(&file, "file").unwrap();
    let valid = serde_json::json!({"action":"cd", "path":"/"}).to_string();
    let cases = [
        ("".to_owned(), "0", false),
        (" \n\t".to_owned(), "0", false),
        (valid.clone(), "7", true),
        ("{".to_owned(), "0", true),
        ("null".to_owned(), "0", true),
        ("[]".to_owned(), "0", true),
        ("{}".to_owned(), "0", true),
        (r#"{"action":"cd"}"#.to_owned(), "0", true),
        (r#"{"action":"cd","path":12}"#.to_owned(), "0", true),
        (r#"{"action":false,"path":"/"}"#.to_owned(), "0", true),
        (r#"{"action":"open","path":"/"}"#.to_owned(), "0", true),
        (r#"{"action":"cd","path":"relative"}"#.to_owned(), "0", true),
        (
            r#"{"action":"cd","path":"/","extra":true}"#.to_owned(),
            "0",
            true,
        ),
        (format!("{valid}\n{valid}"), "0", true),
        (
            serde_json::json!({"action":"cd", "path":file}).to_string(),
            "0",
            true,
        ),
        (
            serde_json::json!({"action":"cd", "path":root.path().join("missing")}).to_string(),
            "0",
            true,
        ),
    ];
    for (stdout, status, error) in cases {
        let record = root.path().join("result.json");
        fs::write(&record, &stdout).unwrap();
        let output = Command::new("nu")
            .args(["--no-config-file", "check.nu"])
            .current_dir(root.path())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    root.path().display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .env("DIRPICK_TEST_STDOUT", &record)
            .env("DIRPICK_TEST_STATUS", status)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let pwd: String = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            pwd,
            root.path().to_str().unwrap(),
            "output={stdout:?}, status={status}"
        );
        assert_eq!(
            !output.stderr.is_empty(),
            error,
            "output={stdout:?}, stderr={:?}",
            output.stderr
        );
    }
}
