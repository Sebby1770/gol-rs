use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn gol() -> Command {
    Command::new(env!("CARGO_BIN_EXE_gol"))
}

#[test]
fn pattern_catalogue_is_available() {
    let output = gol().arg("patterns").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("gosper-glider-gun"));
    assert!(stdout.contains("r-pentomino"));
}

#[test]
fn headless_cycle_detection_stops_on_exact_repeat() {
    let output = gol()
        .args([
            "--pattern",
            "blinker",
            "--width",
            "9",
            "--height",
            "9",
            "--gens",
            "20",
            "--headless",
            "--stop-on-cycle",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"period\":2"));
    assert!(stdout.contains("\"final_generation\":2"));
    assert!(!stdout.contains("\"generation\":3"));
}

#[test]
fn piped_default_output_is_plain_and_contains_no_ansi() {
    let output = gol()
        .args([
            "--pattern",
            "block",
            "--width",
            "4",
            "--height",
            "4",
            "--gens",
            "0",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!output.stdout.contains(&0x1b));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("generation=0 population=4"));
}

#[test]
fn terminal_mode_never_hides_the_cursor() {
    let output = gol()
        .args([
            "--pattern",
            "block",
            "--width",
            "4",
            "--height",
            "4",
            "--gens",
            "0",
            "--output",
            "terminal",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!output.stdout.windows(4).any(|window| window == b"?25l"));
}

#[test]
fn reads_rle_from_stdin() {
    let mut child = gol()
        .args([
            "--rle",
            "-",
            "--width",
            "5",
            "--height",
            "5",
            "--gens",
            "0",
            "--headless",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"x = 3, y = 3, rule = B3/S23\nbob$2bo$3o!\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("\"population\":5")
    );
}

#[test]
fn invalid_options_have_usage_exit_code() {
    let output = gol()
        .args(["--pattern", "glider", "--rle", "pattern.rle"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--pattern conflicts with --rle")
    );
}

#[test]
fn exports_final_state_as_valid_rle() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("gol-rs-cli-{}-{unique}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let output_path = directory.join("final.rle");
    let output = gol()
        .args([
            "--pattern",
            "block",
            "--width",
            "6",
            "--height",
            "6",
            "--gens",
            "1",
            "--headless",
            "--export-rle",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let exported = fs::read_to_string(&output_path).unwrap();
    assert!(exported.contains("x = 2, y = 2, rule = B3/S23"));
    assert!(exported.contains("2o$2o!"));
    fs::remove_dir_all(directory).unwrap();
}
