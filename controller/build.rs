use std::{
    io,
    process::Command,
};

use chrono::Utc;
use winres::WindowsResource;

fn main() -> io::Result<()> {
    {
        let git_hash = String::from_utf8(
            Command::new("git")
                .args(&["rev-parse", "HEAD"])
                .output()?
                .stdout,
        )
        .expect("the git hash to be utf-8");

        let build_time = Utc::now().to_string();

        println!("cargo:rustc-env=GIT_HASH={}", &git_hash[0..7]);
        println!("cargo:rustc-env=BUILD_TIME={}", build_time);
    }

    {
        let mut resource = WindowsResource::new();
        resource.set_icon("./resources/app-icon.ico");
        resource.compile()?;
    }
    Ok(())
}
