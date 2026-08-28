//! Application state.
//!
//! One session behind one lock. Every command takes the lock, does its work and
//! releases it. PDF work is fast enough that this never blocks the UI
//! noticeably, and it keeps the model free of interior mutability.

use std::sync::Arc;

use npdf_core::Session;
use parking_lot::Mutex;
use tauri::{App, Manager, Runtime};

use crate::platform;

pub struct AppState {
    session: Mutex<Session>,
}

impl AppState {
    pub fn new<R: Runtime>(app: &App<R>) -> Result<Self, Box<dyn std::error::Error>> {
        let services = platform::services(app.handle())?;
        Ok(Self {
            session: Mutex::new(Session::new(Arc::from(services))),
        })
    }

    pub fn session(&self) -> parking_lot::MutexGuard<'_, Session> {
        self.session.lock()
    }
}

/// Convenience for the command functions.
pub trait SessionAccess {
    fn npdf(&self) -> parking_lot::MutexGuard<'_, Session>;
}

impl<R: Runtime> SessionAccess for tauri::AppHandle<R> {
    fn npdf(&self) -> parking_lot::MutexGuard<'_, Session> {
        self.state::<AppState>().inner().session()
    }
}
