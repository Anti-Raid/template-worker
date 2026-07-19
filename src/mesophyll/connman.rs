use std::{env::temp_dir, fs::{DirBuilder, remove_dir_all, remove_file}, os::unix::fs::DirBuilderExt, path::PathBuf};

pub fn new_sockfile(iname: String, name: String) -> Result<SockFile, crate::Error> {
    let dir = temp_dir().join(&iname);

    DirBuilder::new()
        .recursive(true)
        .mode(0o700) 
        .create(&dir)?;

    let sock = dir.join(&name);
    Ok(SockFile { dir, sock, del_on_drop: true })
}

pub fn new_sockfile_rooted(dir: PathBuf, name: String) -> Result<SockFile, crate::Error> {
    let sock = dir.join(&name);
    Ok(SockFile { dir, sock, del_on_drop: true })
}

#[derive(Debug)]
pub struct SockFile {
    pub dir: PathBuf,
    pub sock: PathBuf,
    del_on_drop: bool
}

impl SockFile {
    pub fn from_env(dir: PathBuf, sock: PathBuf) -> Self {
        Self { dir, sock, del_on_drop: false }
    }

    pub fn drop_full(&self) {
        log::info!("Dropping dir {:?}", self.dir);
        let _ = remove_dir_all(&self.dir);
    }

}

// TODO: This never seems to actually hit
impl Drop for SockFile {
    fn drop(&mut self) {
        if self.del_on_drop {
            log::info!("Dropping socket {:?}", self.sock);
            let _ = remove_file(&self.sock);
        }
    }
}

// Avoid unsupported configurations
#[cfg(not(unix))]
compile_error!("Mesophyll conn-man is currently only supported on Unix-like operating systems. Please file a github issue if you rely on non-Unix support");
