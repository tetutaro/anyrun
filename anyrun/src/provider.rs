use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
};

use anyrun_provider_ipc as ipc;
use relm4::Sender;
use tokio::{net::UnixListener, sync::mpsc::Receiver};

use crate::config::Config;

pub fn worker(
    config: Arc<Config>,
    config_dir: Option<String>,
    mut rx: Receiver<anyrun_provider_ipc::Request>,
    sender: Sender<anyrun_provider_ipc::Response>,
    // The stdin received by the launching command
    stdin: Vec<u8>,
    // The environment of the launching command
    env: Vec<(String, String)>,
) -> io::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let socket_path = format!(
                "{}/anyrun.sock",
                env::var("XDG_RUNTIME_DIR").unwrap_or("/tmp".to_string())
            );
            // Make sure that it does not exist already
            let _ = fs::remove_file(&socket_path);
            let listener = UnixListener::bind(&socket_path).unwrap();

            let provider = expand_home(&config.provider);
            let mut child = match Command::new(&provider)
                .stdin(Stdio::piped())
                .arg("--config-dir")
                .arg(config_dir.unwrap_or(ipc::CONFIG_DIRS[0].to_string()))
                .args(
                    config
                        .plugins
                        .iter()
                        .flat_map(|plugin| [PathBuf::from("-p"), plugin.to_owned()]),
                )
                .arg("connect-to")
                .arg(&socket_path)
                .envs(env)
                .spawn()
            {
                Ok(child) => child,
                Err(why) => match why.kind() {
                    io::ErrorKind::NotFound => {
                        eprintln!("[anyrun] `{}` Not found, make sure `anyrun-provider` is installed and available in $PATH, \
                             or configure an alternative path via the `provider` config option.", provider.display());
                        return Ok(());
                    }
                    _ => return Err(why),
                },
            };

            if let Some(mut child_stdin) = child.stdin.take() {
                child_stdin.write_all(&stdin).unwrap();
            };

            let (stream, _) = listener.accept().await?;
            let mut socket = ipc::Socket::new(stream);

            loop {
                tokio::select! {
                    req = rx.recv() => {
                        if let Some(req) = req {
                        socket.send(&req).await?;
                        if matches!(req, ipc::Request::Quit) {
                            break;
                        }
                        }
                    }
                    res = socket.recv() => {
                        match res {
                    Ok(response) => sender.emit(response),
                    Err(why) => {
                        eprintln!("[anyrun] Error reading from IPC: {why}");
                        break;
                    },
                        }
                    }
                }
            }

            // Remove it after we are done with it
            let _ = fs::remove_file(&socket_path);
            // Make sure it exits properly and doesn't leave a zombie process
            let _ = child.wait();

            Ok(())
        }
    )
}

/// Expand a leading `~`, `$HOME` or `${HOME}` in the path to the home directory
fn expand_home(path: &Path) -> PathBuf {
    let (Ok(home), Some(path_str)) = (env::var("HOME"), path.to_str()) else {
        return path.to_owned();
    };

    ["~", "$HOME", "${HOME}"]
        .iter()
        .find_map(|prefix| {
            path_str
                .strip_prefix(prefix)
                .filter(|rest| rest.is_empty() || rest.starts_with('/'))
                .map(|rest| PathBuf::from(format!("{home}{rest}")))
        })
        .unwrap_or_else(|| path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_home_prefix() {
        let home = env::var("HOME").unwrap();
        for input in ["~", "$HOME", "${HOME}"] {
            assert_eq!(expand_home(Path::new(input)), PathBuf::from(&home));
            assert_eq!(
                expand_home(Path::new(&format!("{input}/.cargo/bin/anyrun-provider"))),
                PathBuf::from(format!("{home}/.cargo/bin/anyrun-provider"))
            );
        }
        for input in [
            "anyrun-provider",
            "/usr/bin/anyrun-provider",
            "~user/bin",
            "$HOMEDIR/x",
        ] {
            assert_eq!(expand_home(Path::new(input)), PathBuf::from(input));
        }
    }
}
