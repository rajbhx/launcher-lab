//! Background data loader: one worker thread between `platform` and the UI.
//!
//! `platform` calls are JNI round-trips (package queries, `Bitmap` decoding,
//! wallpaper draws). Running them on the render thread would stall frames, so
//! everything goes through a single worker thread:
//!
//! * the UI sends [`Request`]s (fire and forget, the channel carries owned
//!   values only),
//! * the worker calls the matching `platform` function and sends back a
//!   [`Reply`] with either the data or the structured error,
//! * the UI drains replies once per frame and converts images into textures
//!   on the render thread (`egui` textures must be created there).
//!
//! The worker never panics: every call is additionally wrapped in
//! `catch_unwind` so even a hypothetical defect in one request leaves the
//! loader alive and reports an error the UI can toast. Dropping the `Loader`
//! (app shutdown) closes the channel and the worker exits on its next receive.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::platform::{self, ImageRgba, PlatformError, RealApp};

/// One unit of work for the loader thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Read every launcher activity on the device.
    Apps,
    /// Decode one app icon (physical-pixel edge).
    Icon { package: String, size: u32 },
    /// Read the system wallpaper (physical-pixel target box).
    Wallpaper { width: u32, height: u32 },
}

/// The outcome of one [`Request`].
#[derive(Debug)]
pub enum Reply {
    Apps(Result<Vec<RealApp>, PlatformError>),
    Icon {
        package: String,
        result: Result<ImageRgba, PlatformError>,
    },
    Wallpaper(Result<ImageRgba, PlatformError>),
}

/// Handle to the loader thread (owned by the launcher state).
pub struct Loader {
    tx: Sender<Request>,
    rx: Receiver<Reply>,
}

impl Loader {
    /// Spawn the worker thread. `None` when the OS refuses a thread.
    pub fn spawn() -> Option<Self> {
        let (request_tx, request_rx) = mpsc::channel::<Request>();
        let (reply_tx, reply_rx) = mpsc::channel::<Reply>();
        let spawned = thread::Builder::new()
            .name("launcher-lab-data".to_owned())
            .spawn(move || worker(request_rx, reply_tx));
        match spawned {
            Ok(_handle) => Some(Self {
                tx: request_tx,
                rx: reply_rx,
            }),
            Err(error) => {
                log::error!("loader: could not spawn the data thread: {error}");
                None
            }
        }
    }

    /// Queue one request. Fails only when the worker is gone.
    pub fn request(&self, request: Request) -> Result<(), PlatformError> {
        self.tx
            .send(request)
            .map_err(|_| PlatformError::NotInitialized)
    }

    /// Collect up to `max` finished replies without blocking.
    pub fn drain(&self, max: usize) -> Vec<Reply> {
        let mut replies = Vec::new();
        for _ in 0..max {
            match self.rx.try_recv() {
                Ok(reply) => replies.push(reply),
                Err(_) => break,
            }
        }
        replies
    }
}

/// The worker loop: pull a request, run it, push the reply.
fn worker(requests: Receiver<Request>, replies: Sender<Reply>) {
    while let Ok(request) = requests.recv() {
        let reply = match request {
            Request::Apps => Reply::Apps(run(platform::query_launcher_apps)),
            Request::Icon { package, size } => {
                let package_for_call = package.clone();
                let result = run(move || platform::app_icon_rgba(&package_for_call, size));
                Reply::Icon { package, result }
            }
            Request::Wallpaper { width, height } => {
                Reply::Wallpaper(run(move || platform::system_wallpaper_rgba(width, height)))
            }
        };
        if replies.send(reply).is_err() {
            // The launcher state is gone; nothing left to work for.
            break;
        }
    }
}

/// Run one platform call, containing any panic inside an error value.
fn run<T>(call: impl FnOnce() -> Result<T, PlatformError>) -> Result<T, PlatformError> {
    match catch_unwind(AssertUnwindSafe(call)) {
        Ok(result) => result,
        Err(_) => Err(PlatformError::Jni {
            op: "loading device data",
            detail: "an internal error was contained; nothing was changed".to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn loader_reports_unsupported_on_host() {
        let loader = Loader::spawn().expect("loader thread spawns");
        loader.request(Request::Apps).expect("queue apps");
        loader
            .request(Request::Icon {
                package: "com.example.app".to_owned(),
                size: 144,
            })
            .expect("queue icon");
        loader
            .request(Request::Wallpaper {
                width: 1080,
                height: 2340,
            })
            .expect("queue wallpaper");

        let deadline = Instant::now() + Duration::from_secs(15);
        let mut replies: Vec<Reply> = Vec::new();
        while replies.len() < 3 && Instant::now() < deadline {
            replies.extend(loader.drain(8));
            if replies.len() < 3 {
                thread::sleep(Duration::from_millis(10));
            }
        }
        assert_eq!(replies.len(), 3, "all three replies must arrive");
        assert!(matches!(
            &replies[0],
            Reply::Apps(Err(PlatformError::Unsupported))
        ));
        match &replies[1] {
            Reply::Icon { package, result } => {
                assert_eq!(package, "com.example.app");
                assert!(matches!(result, Err(PlatformError::Unsupported)));
            }
            other => panic!("expected an icon reply, got {other:?}"),
        }
        assert!(matches!(
            &replies[2],
            Reply::Wallpaper(Err(PlatformError::Unsupported))
        ));
    }

    #[test]
    fn icon_replies_keep_their_package() {
        let loader = Loader::spawn().expect("loader thread spawns");
        for package in ["com.alpha.one", "com.beta.two"] {
            loader
                .request(Request::Icon {
                    package: package.to_owned(),
                    size: 144,
                })
                .expect("queue icon");
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut packages: Vec<String> = Vec::new();
        while packages.len() < 2 && Instant::now() < deadline {
            for reply in loader.drain(8) {
                if let Reply::Icon { package, .. } = reply {
                    packages.push(package);
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(packages, vec!["com.alpha.one", "com.beta.two"]);
    }
}
