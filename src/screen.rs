use crate::config_manager::ConfigManager;
use rusttype::Font;
use std::fmt::Debug;
use std::sync::{atomic::AtomicBool, Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Screen {
    pub description: String,
    pub bytes: Arc<Mutex<Vec<u8>>>,
    pub font: Arc<Mutex<Option<Font<'static>>>>,
    pub active: Arc<AtomicBool>,
    pub initial_update_called: Arc<AtomicBool>,
    pub handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    pub mode: Arc<Mutex<u32>>,
    pub mode_timeout: Arc<Mutex<Option<Instant>>>,
    pub config: Arc<RwLock<ConfigManager>>,
}

impl Default for Screen {
    fn default() -> Screen {
        Screen {
            description: String::from(""),
            bytes: Arc::new(Mutex::new(Vec::new())),
            font: Arc::new(Mutex::new(Font::try_from_vec(Vec::from(
                include_bytes!("Liberation.ttf") as &[u8],
            )))),
            active: Arc::new(AtomicBool::new(false)),
            initial_update_called: Arc::new(AtomicBool::new(false)),
            handle: Arc::new(Mutex::new(None)),
            mode: Arc::new(Mutex::new(0)),
            mode_timeout: Arc::new(Mutex::new(Some(Instant::now()))),
            config: Arc::new(RwLock::new(ConfigManager::new(None))),
        }
    }
}

impl std::fmt::Debug for dyn BasicScreen {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

pub trait BasicScreen {
    fn update(&mut self) -> ();
    fn description(&self) -> String;
    fn current_image(&self) -> Vec<u8>;
    fn initial_update_called(&mut self) -> bool;
    fn start(&self) -> ();
    fn stop(&self) -> ();
    fn set_mode_for_short(&mut self, _mode: u32) {}
    fn enabled(&self) -> bool;
    fn set_status(&self, status: bool) -> ();
}

pub trait ScreenControl {
    fn start_worker(&self);
    fn stop_worker(&self);
}

impl ScreenControl for Screen {
    fn start_worker(&self) {
        self.active
            .store(true, std::sync::atomic::Ordering::Release);
        match self.handle.lock() {
            Ok(lock) => match lock.as_ref() {
                Some(handle) => {
                    handle.thread().unpark();
                }
                None => {}
            },
            Err(_) => {}
        }
    }

    fn stop_worker(&self) {
        self.active
            .store(false, std::sync::atomic::Ordering::Release);
    }
}
