use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let host = env::var_os("HOST")
        .map(|host| host.into_string().unwrap())
        .unwrap();
    let out_dir = env::var_os("OUT_DIR")
        .map(|s| PathBuf::from(s).join("libpcap"))
        .unwrap();
    let build_dir = out_dir.join("build");
    let install_dir = out_dir.join("install");
    let libdir = install_dir.join("lib");

    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")
        .map(|s| PathBuf::from(s))
        .unwrap();

    // The directory where the includes installed by libpcap can be found.
    let include_dir = install_dir.join("include").join("htp");

    // Copy the source into a build directory as we shouldn't build in the
    // source directory.
    let _ = std::fs::remove_dir_all(&build_dir);
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut opts = fs_extra::dir::CopyOptions::new();
    opts.copy_inside = true;
    fs_extra::dir::copy(manifest_dir.join("libpcap"), &build_dir, &opts).unwrap();

    // The above copy can mess up the timestamps causing autoconf to autoreconf
    // itself, which we do not want. So touch all the files.
    let now = std::time::SystemTime::now();
    let now = filetime::FileTime::from_system_time(now);
    for entry in walkdir::WalkDir::new(&build_dir) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            filetime::set_file_times(entry.path(), now, now).expect("Failed to set file time");
        }
    }

    let shell = if host.contains("pc-windows-gnu") {
        // Need better detection...
        "\\msys64\\usr\\bin\\bash.exe"
    } else {
        "/bin/sh"
    };

    let prefix = if host.contains("pc-windows-gnu") {
        fix_windows_path(&install_dir)
    } else {
        install_dir.display().to_string()
    };

    // Clear some environment variables that can mess these up.
    env::remove_var("MAKEFLAGS");
    env::remove_var("MFLAGS");

    Command::new(shell)
        .arg("./configure")
        .arg(&format!("--prefix={}", prefix))
        .arg("--disable-shared")
        .current_dir(&build_dir)
        .spawn()
        .expect("./configure failed")
        .wait()
        .expect("./configure failed");
    Command::new("make")
        .current_dir(&build_dir)
        .spawn()
        .expect("make failed")
        .wait()
        .expect("make failed");
    Command::new("make")
        .arg("install")
        .current_dir(&build_dir)
        .spawn()
        .expect("make failed")
        .wait()
        .expect("make failed");

    println!("cargo:rustc-link-lib=static=pcap");
    println!("cargo:rustc-link-search=native={}", libdir.display());
    //println!("cargo:rustc-link-lib=z");

    // Copy the header files into target/include so they can easily be referenced.
    let cargo_target_dir = env::var_os("CARGO_TARGET_DIR").map(|s| PathBuf::from(s));
    if let Some(target_dir) = cargo_target_dir {
        let include_target_dir = target_dir.join("include");
        let opts = fs_extra::dir::CopyOptions::new();
        fs_extra::dir::create_all(&include_target_dir, true)
            .expect("Failed to create include directory");
        fs_extra::dir::copy(&include_dir, &include_target_dir, &opts)
            .expect("Failed to copy include files");
    }
}

// Fixup the Windows path. Taken from
// https://github.com/alexcrichton/openssl-src-rs/blob/master/src/lib.rs#L434
fn fix_windows_path(path: &Path) -> String {
    if !cfg!(windows) {
        return path.to_str().unwrap().to_string();
    }
    let path = path.to_str().unwrap().replace("\\", "/");
    return change_drive(&path).unwrap_or(path);

    fn change_drive(s: &str) -> Option<String> {
        let mut ch = s.chars();
        let drive = ch.next().unwrap_or('C');
        if ch.next() != Some(':') {
            return None;
        }
        if ch.next() != Some('/') {
            return None;
        }
        Some(format!("/{}/{}", drive, &s[drive.len_utf8() + 2..]))
    }
}
