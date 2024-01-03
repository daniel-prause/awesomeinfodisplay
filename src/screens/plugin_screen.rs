use crate::config_manager::ConfigManager;
use crate::screens::{BasicScreen, Screen, Screenable};
use crossbeam_channel::{bounded, Receiver, Sender};
use image::{ImageBuffer, Rgb, RgbImage};
use imageproc::drawing::draw_text_mut;
use rusttype::{Font, Scale};
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::path::PathBuf;
use std::{
    rc::Rc,
    sync::{atomic::AtomicBool, atomic::Ordering, Arc, RwLock},
    thread,
    time::Duration,
};

pub struct PluginScreen {
    screen: Screen,
    receiver: Receiver<ExchangeFormat>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExchangeFormat {
    pub texts: Vec<Text>,
    pub images: Vec<Image>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Text {
    pub value: String,
    pub x: i32,
    pub y: i32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub color: Vec<u8>, // RGB
    pub symbol: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Image {
    pub value: Vec<u8>,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Default for ExchangeFormat {
    fn default() -> ExchangeFormat {
        ExchangeFormat {
            texts: vec![],
            images: vec![],
        }
    }
}

impl Screenable for PluginScreen {
    fn get_screen(&mut self) -> &mut Screen {
        &mut self.screen
    }
}

impl BasicScreen for PluginScreen {
    fn update(&mut self) {
        let exchange_format = self.receiver.try_recv();
        match exchange_format {
            Ok(state) => {
                self.draw_screen(state);
            }
            Err(_) => {}
        }
    }
}

// TODO: think about multiple exchange formats for different devices
impl PluginScreen {
    fn draw_screen(&mut self, exchange_format: ExchangeFormat) {
        let mut image = RgbImage::new(256, 64);
        self.draw_exchange_format(&mut image, exchange_format);
        self.screen.main_screen_bytes = image.into_vec();
    }

    pub fn draw_exchange_format(
        &mut self,
        image: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
        exchange_format: ExchangeFormat,
    ) {
        // draw text parts
        for text in exchange_format.texts.iter() {
            // determine color
            let color;
            if text.color.len() != 3 {
                color = Rgb([text.color[0], text.color[1], text.color[2]]);
            } else {
                color = Rgb([255, 255, 255])
            }

            // determine font
            let font;
            if text.symbol {
                font = &self.screen.symbols;
            } else {
                font = &self.screen.font;
            }
            // draw text
            draw_text_mut(
                image,
                color,
                text.x,
                text.y,
                Scale {
                    x: text.scale_x,
                    y: text.scale_y,
                },
                font,
                &text.value,
            );
        }
        // TODO: draw images!
    }

    pub fn new(
        font: Rc<Font<'static>>,
        symbols: Rc<Font<'static>>,
        config_manager: Arc<RwLock<ConfigManager>>,
        library_path: PathBuf,
    ) -> PluginScreen {
        let (tx, rx): (Sender<ExchangeFormat>, Receiver<ExchangeFormat>) = bounded(1);
        // load library:

        let active = Arc::new(AtomicBool::new(false));
        // load library
        let get_key: libloading::Symbol<unsafe extern "C" fn() -> *mut i8>;
        let get_description: libloading::Symbol<unsafe extern "C" fn() -> *mut i8>;
        let lib: Arc<libloading::Library>;
        unsafe {
            lib = Arc::new(libloading::Library::new(library_path).expect("Failed to load library"));
            get_key = lib.get(b"get_key").expect("Get key not found!");
            get_description = lib
                .get(b"get_description")
                .expect("Get description not found!");
        }
        let this = PluginScreen {
            screen: Screen {
                description: unsafe {
                    CString::from_raw(get_description())
                        .to_owned()
                        .to_string_lossy()
                        .to_string()
                },
                key: unsafe {
                    CString::from_raw(get_key())
                        .to_owned()
                        .to_string_lossy()
                        .to_string()
                },
                font,
                symbols,
                active: active.clone(),
                handle: Some(thread::spawn(move || {
                    let sender = tx.to_owned();
                    let active = active;
                    let lib: Arc<libloading::Library> = lib.clone();
                    loop {
                        while !active.load(Ordering::Acquire) {
                            thread::park();
                        }
                        unsafe {
                            let get_screen: libloading::Symbol<unsafe extern "C" fn() -> *mut i8> =
                                lib.get(b"get_screen").expect("Function gone :(");
                            let data = CString::from_raw(get_screen()); // TODO: give copy of config to get_screen
                            let exchange_format =
                                serde_json::from_str(&data.to_str().unwrap_or_default())
                                    .unwrap_or_default();
                            sender.try_send(exchange_format).unwrap_or_default();
                        }
                        thread::sleep(Duration::from_millis(1000));
                    }
                })),
                config_manager,
                ..Default::default()
            },
            receiver: rx,
        };
        this
    }
}