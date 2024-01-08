use crate::config_manager::ConfigManager;
use crate::screens::{BasicScreen, Screen, Screenable};
use crossbeam_channel::{bounded, Receiver, Sender};
use exchange_format::*;
use image::{EncodableLayout, GenericImage, ImageBuffer, Rgb, RgbImage};
use imageproc::drawing::draw_text_mut;
use libloading::Library;
use rusttype::{Font, Scale};
use std::ffi::CString;
use std::path::PathBuf;
use std::{
    rc::Rc,
    sync::{atomic::AtomicBool, atomic::Ordering, Arc, RwLock},
    thread,
    time::Duration,
};

struct Lib {
    library: Library,
}

impl Lib {
    fn new(path_buf: PathBuf) -> Lib {
        Lib {
            library: unsafe { libloading::Library::new(path_buf).expect("Failed to load library") },
        }
    }

    fn get_key(&self) -> String {
        let get_key: libloading::Symbol<unsafe extern "C" fn() -> *mut i8> =
            unsafe { self.library.get(b"get_key").expect("Get key not found!") };
        unsafe {
            CString::from_raw(get_key())
                .to_owned()
                .to_string_lossy()
                .to_string()
        }
    }

    fn get_description(&self) -> String {
        let get_description: libloading::Symbol<unsafe extern "C" fn() -> *mut i8> = unsafe {
            self.library
                .get(b"get_description")
                .expect("Get key not found!")
        };
        unsafe {
            CString::from_raw(get_description())
                .to_owned()
                .to_string_lossy()
                .to_string()
        }
    }

    fn get_config_layout(&self) -> ExchangeableConfig {
        let get_config_layout: libloading::Symbol<unsafe extern "C" fn() -> *mut i8> = unsafe {
            self.library
                .get(b"get_config_layout")
                .expect("Get config layout not found!")
        };

        unsafe {
            ExchangeableConfig::from(
                CString::from_raw(get_config_layout())
                    .to_owned()
                    .to_string_lossy()
                    .to_string(),
            )
        }
    }

    fn get_screen(&self) -> ExchangeFormat {
        let get_screen: libloading::Symbol<unsafe extern "C" fn() -> *mut i8> = unsafe {
            self.library
                .get(b"get_screen")
                .expect("Get screen not found!")
        };

        unsafe {
            serde_json::from_str(
                CString::from_raw(get_screen())
                    .to_owned()
                    .to_string_lossy()
                    .to_string()
                    .as_str(),
            )
            .unwrap_or_default()
        }
    }
}

pub struct PluginScreen {
    screen: Screen,
    receiver: Receiver<ExchangeFormat>,
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
        for item in exchange_format.items.iter() {
            match item {
                Item::Text(text) => {
                    // determine color
                    let color;
                    if text.color.len() != 3 {
                        color = Rgb([text.color[0], text.color[1], text.color[2]]);
                    } else {
                        color = Rgb([255, 255, 255])
                    }

                    // determine font
                    let font = if text.symbol {
                        &self.screen.symbols
                    } else {
                        &self.screen.font
                    };

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
                Item::Image(overlay_image) => {
                    let mut overlay = RgbImage::new(overlay_image.width, overlay_image.height);
                    overlay.copy_from_slice(overlay_image.value.as_bytes());
                    image
                        .copy_from(&overlay, overlay_image.x, overlay_image.y)
                        .unwrap_or_default();
                }
            }
        }
    }

    pub fn new(
        font: Rc<Font<'static>>,
        symbols: Rc<Font<'static>>,
        config_manager: Arc<RwLock<ConfigManager>>,
        library_path: PathBuf,
    ) -> PluginScreen {
        let (tx, rx): (Sender<ExchangeFormat>, Receiver<ExchangeFormat>) = bounded(1);
        let active = Arc::new(AtomicBool::new(false));

        // load library
        let lib: Lib = Lib::new(library_path);
        let mut this = PluginScreen {
            screen: Screen {
                description: lib.get_description(),
                key: lib.get_key(),
                config_layout: lib.get_config_layout(),
                font,
                symbols,
                config_manager: config_manager.clone(),
                active: active.clone(),
                handle: Some(thread::spawn(move || {
                    let sender = tx.to_owned();
                    let active = active;

                    loop {
                        while !active.load(Ordering::Acquire) {
                            thread::park();
                        }

                        let serialized_screen_config = config_manager
                            .read()
                            .unwrap()
                            .get_screen_config(&lib.get_key())
                            .to_raw();
                        set_current_config(serialized_screen_config);

                        let exchange_format = lib.get_screen();

                        sender.try_send(exchange_format).unwrap_or_default();
                        thread::sleep(Duration::from_millis(1000));
                    }
                })),
                ..Default::default()
            },
            receiver: rx,
        };
        this.draw_screen(ExchangeFormat::default());
        this
    }
}
