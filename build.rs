fn main() {
    println!("cargo:rerun-if-changed=.env");
    if let Ok(read_vars) = dotenvy::dotenv_iter() {
        for (key, value) in read_vars.map(Result::unwrap) {
            println!("cargo:rustc-env={key}={value}");
        }
    }

    println!("cargo:rustc-env=HUMMINGBIRD_VERSION_SUFFIX={}", 'sfx: {
        if let Ok(v) = std::env::var("HUMMINGBIRD_VERSION_SUFFIX") {
            break 'sfx v;
        }

        println!("cargo:rerun-if-changed=RELEASE_CHANNEL");
        let chan = std::fs::read_to_string("RELEASE_CHANNEL");
        let chan = chan.as_deref().map(str::trim_ascii);
        if let Ok("stable") = chan {
            break 'sfx " (release)".to_owned();
        } // no need for `.git` for stable releases

        println!("cargo:rerun-if-changed=.git/logs/HEAD");
        let git = std::process::Command::new("git")
            .arg("--git-dir=.git")
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("failed to run git: is it installed?");
        if git.status.success()
            && let output = git.stdout.trim_ascii_end()
            && output.iter().all(u8::is_ascii_hexdigit)
            && let Ok(sha) = std::str::from_utf8(output)
        {
            let sha = &sha[..7];
            match chan {
                Err(_) | Ok("dev") => format!("-dev ({sha})"),
                // Ok(b"preview") // pre-release?
                // Ok(b"nightly") // ci build?
                Ok(chan) => panic!("invalid release channel '{chan}'"),
            }
        } else {
            panic!(
                "git returned an error: `git rev-parse HEAD` exited with {}\
                \n== stdout ==\n{}\n== stderr ==\n{}",
                git.status,
                String::from_utf8_lossy(&git.stdout),
                String::from_utf8_lossy(&git.stderr),
            );
        }
    });
}
