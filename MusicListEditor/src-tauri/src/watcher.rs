use std::path::PathBuf;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

/// 监听工作区内 .m3u 的增删改（F2.6），400ms 去抖后通知前端
pub struct WorkspaceWatcher {
    inner: Option<notify::RecommendedWatcher>,
}

impl Default for WorkspaceWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceWatcher {
    pub fn new() -> Self {
        Self { inner: None }
    }

    pub fn watch(&mut self, app: AppHandle, root: PathBuf) {
        self.stop();

        let (sender, receiver) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        }) {
            Ok(value) => value,
            Err(_) => return,
        };
        if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
            return;
        }
        self.inner = Some(watcher);

        std::thread::spawn(move || loop {
            match receiver.recv() {
                Ok(Ok(event)) => {
                    let relevant = event.paths.iter().any(|path| {
                        path.extension()
                            .map(|value| {
                                let text = value.to_string_lossy().to_lowercase();
                                text == "m3u" || text == "m3u8"
                            })
                            .unwrap_or(false)
                    });
                    if !relevant {
                        continue;
                    }
                    std::thread::sleep(Duration::from_millis(400));
                    while receiver.try_recv().is_ok() {}
                    let _ = app.emit("fs:changed", ());
                }
                Ok(Err(_)) => {}
                Err(_) => return,
            }
        });
    }

    pub fn stop(&mut self) {
        self.inner = None;
    }
}
