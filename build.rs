use vergen_git2::{Emitter, Git2Builder};

fn read_or_create_envfile() -> std::io::Result<String> {
    std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(".env")
        .and_then(std::io::read_to_string)
}

fn main() -> anyhow::Result<()> {
    let envfile = read_or_create_envfile()?;
    println!("cargo:rerun-if-changed=.env");
    dotenvy::from_read(envfile.as_bytes())?;
    let mut vars = dotenvy::from_read_iter(envfile.as_bytes());
    while let Some((key, _)) = vars.next().transpose()? {
        println!("cargo:rustc-env={key}={}", std::env::var(&key)?);
    }

    let flags = Git2Builder::default().sha(true).build()?;
    Emitter::default().add_instructions(&flags)?.emit()
}
