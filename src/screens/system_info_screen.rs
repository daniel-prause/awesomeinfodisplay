extern crate cpu_monitor;
use crate::config_manager::ConfigManager;
use crate::screens::BasicScreen;
use crate::screens::Screen;
use crate::screens::ScreenControl;
use cpu_monitor::CpuInstant;
use crossbeam_channel::bounded;
use crossbeam_channel::{Receiver, Sender};
use image::{ImageBuffer, Rgb, RgbImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_hollow_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use rusttype::Font;
use rusttype::Scale;
use std::rc::Rc;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc, RwLock};
use std::thread;
use std::time::Duration;
use systemstat::{saturating_sub_bytes, Platform, System};

pub struct SystemInfoScreen {
    screen: Screen,
    receiver: Receiver<SystemInfoState>,
}

#[derive(Default)]
struct SystemInfoState {
    cpu_usage: f64,
    ram_usage: f64,
}

impl BasicScreen for SystemInfoScreen {
    fn description(&self) -> &String {
        &self.screen.description
    }

    fn current_image(&self) -> &Vec<u8> {
        self.screen.current_image()
    }

    fn update(&mut self) {
        SystemInfoScreen::update(self)
    }

    fn start(&self) {
        self.screen.start_worker();
    }

    fn stop(&self) {
        self.screen.stop_worker();
    }

    fn key(&self) -> &String {
        &self.screen.key()
    }

    fn initial_update_called(&mut self) -> bool {
        self.screen.initial_update_called()
    }

    fn enabled(&self) -> bool {
        self.screen
            .config_manager
            .read()
            .unwrap()
            .config
            .system_info_screen_active
    }

    fn set_status(&self, status: bool) {
        self.screen
            .config_manager
            .write()
            .unwrap()
            .config
            .system_info_screen_active = status;
    }
}

impl SystemInfoScreen {
    pub fn draw_cpu(
        &mut self,
        image: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
        cpu_usage: f64,
        scale: Scale,
    ) {
        let cpu_text = format!("{: >3}%", cpu_usage.to_string());
        draw_text_mut(
            image,
            Rgb([255u8, 255u8, 255u8]),
            0,
            0,
            scale,
            &self.screen.font,
            "CPU",
        );
        draw_text_mut(
            image,
            Rgb([255u8, 255u8, 255u8]),
            222,
            0,
            scale,
            &self.screen.font,
            &cpu_text,
        );
        draw_hollow_rect_mut(
            image,
            Rect::at(0, 16).of_size(256, 10),
            Rgb([255u8, 255u8, 255u8]),
        );

        let cpu_filled = ((cpu_usage * 2.56) + 1.0).floor() as u32;
        draw_filled_rect_mut(
            image,
            Rect::at(0, 16).of_size(cpu_filled, 10),
            Rgb([255u8, 255u8, 255u8]),
        );
    }
    pub fn draw_memory(
        &mut self,
        image: &mut ImageBuffer<Rgb<u8>, Vec<u8>>,
        ram_usage: f64,
        scale: Scale,
    ) {
        let memory_text = format!("{: >3}%", ram_usage.to_string());
        draw_text_mut(
            image,
            Rgb([255u8, 255u8, 255u8]),
            0,
            30,
            scale,
            &self.screen.font,
            "RAM",
        );
        draw_text_mut(
            image,
            Rgb([255u8, 255u8, 255u8]),
            222,
            30,
            scale,
            &self.screen.font,
            &memory_text,
        );
        draw_hollow_rect_mut(
            image,
            Rect::at(0, 48).of_size(256, 10),
            Rgb([255u8, 255u8, 255u8]),
        );

        let memory_filled = ((ram_usage * 2.56) + 1.0).floor() as u32;
        draw_filled_rect_mut(
            image,
            Rect::at(0, 48).of_size(memory_filled, 10),
            Rgb([255u8, 255u8, 255u8]),
        );
    }

    fn draw_screen(&mut self, cpu_usage: f64, ram_usage: f64) {
        // draw initial image
        let mut image = RgbImage::new(256, 64);
        let scale = Scale { x: 16.0, y: 16.0 };

        self.draw_cpu(&mut image, cpu_usage, scale);
        self.draw_memory(&mut image, ram_usage, scale);
        self.screen.bytes = image.into_vec();
    }

    pub fn new(
        description: String,
        key: String,
        font: Rc<Font<'static>>,
        config_manager: Arc<RwLock<ConfigManager>>,
    ) -> SystemInfoScreen {
        let (tx, rx): (Sender<SystemInfoState>, Receiver<SystemInfoState>) = bounded(1);
        let active = Arc::new(AtomicBool::new(false));
        let mut this = SystemInfoScreen {
            screen: Screen {
                description,
                key,
                font,
                active: active.clone(),
                handle: Some(thread::spawn(move || {
                    let sys = System::new();
                    let sender = tx.to_owned();
                    let active = active.clone();
                    loop {
                        while !active.load(Ordering::Acquire) {
                            thread::park();
                        }
                        let start = cpu_monitor::CpuInstant::now().unwrap();
                        thread::sleep(Duration::from_millis(1000));
                        let end = CpuInstant::now().unwrap();
                        let duration = end - start;
                        let mut system_info: SystemInfoState = Default::default();
                        system_info.cpu_usage = (duration.non_idle() * 100.0).floor().into();
                        match sys.memory() {
                            Ok(mem) => {
                                system_info.ram_usage =
                                    saturating_sub_bytes(mem.total, mem.free).as_u64() as f64;
                                system_info.ram_usage =
                                    ((system_info.ram_usage / mem.total.as_u64() as f64) * 100.0)
                                        .floor();

                                // we are right now not interested in the error value.
                                // since we only want to have the most recent screen,
                                // it is ok, if screen infos get lost
                                sender.try_send(system_info).unwrap_or_default();
                            }
                            Err(x) => println!("\nMemory: error: {}", x),
                        }
                    }
                })),
                config_manager,
                ..Default::default()
            },
            receiver: rx,
        };

        this.draw_screen(0f64, 0f64);
        this
    }

    pub fn update(&mut self) {
        let system_stats = self.receiver.try_recv();
        match system_stats {
            Ok(system_info_state) => {
                self.draw_screen(system_info_state.cpu_usage, system_info_state.ram_usage);
            }
            Err(_) => {}
        }
    }
}