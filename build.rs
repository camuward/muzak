fn main() {
    if let Ok(read_vars) = dotenvy::dotenv_iter() {
        println!("cargo:rerun-if-changed=.env");
        for (key, value) in read_vars.map(Result::unwrap) {
            println!("cargo:rustc-env={key}={value}");
        }
    }

    println!("cargo:rustc-env=HUMMINGBIRD_VERSION_SUFFIX={}", 'sfx: {
        if let Ok(v) = std::env::var("HUMMINGBIRD_VERSION_SUFFIX") {
            break 'sfx v;
        }

        let chan = std::fs::read_to_string("RELEASE_CHANNEL").inspect(|_| {
            println!("cargo:rerun-if-changed=RELEASE_CHANNEL");
        });

        let chan = chan.as_deref().map(str::trim_ascii).unwrap_or("dev");
        if chan == "stable" {
            break 'sfx " (release)".to_owned();
        }

        // if unstable, include git sha
        println!("cargo:rerun-if-changed=.git/logs/HEAD");
        git(&["rev-parse", "HEAD"], |sha| {
            git(&["status", "--porcelain"], |status| {
                if status.is_empty() {
                    let sha = &sha[..7];
                    match chan {
                        "dev" => format!("-dev ({sha})"),
                        // "preview" // pre-release?
                        // "nightly" // ci build?
                        chan => panic!("invalid release channel '{chan}'"),
                    }
                } else {
                    "-dev (dirty)".to_owned()
                }
            })
        })
    });
}

fn git(args: &[&str], f: impl FnOnce(String) -> String) -> String {
    use std::process::{Command, Output};

    let Ok(cmd @ Output { status, .. }) = Command::new("git")
        .args(["--git-dir=.git", "--work-tree=."])
        .args(args)
        .output()
    else {
        println!("cargo::warning=failed to run git: is git installed?");
        return " (unknown)".to_owned();
    };

    if status.success() {
        f(String::from_utf8_lossy(&cmd.stdout)
            .trim_ascii_end()
            .to_owned())
    } else {
        println!(
            "cargo::warning=git returned an error: `git {}` exited with {status}\
            \n--- git stdout\n{}\n--- git stderr\n{}",
            args.join(" "),
            String::from_utf8_lossy(&cmd.stdout),
            String::from_utf8_lossy(&cmd.stderr),
        );
        " (unknown)".to_owned()
    }
}
