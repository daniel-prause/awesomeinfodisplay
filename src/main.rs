#![windows_subsystem = "windows"]
use iced::text_input::{self, TextInput};
use iced::{
    button, executor, slider, time, window, Align, Application, Button, Checkbox, Column, Command,
    Container, Element, HorizontalAlignment, Image, Length, Row, Settings, Slider, Subscription,
    Text,
};
mod config;
mod config_manager;
mod display_serial_com;
mod screen_manager;
mod screens;
mod style;
use crate::display_serial_com::{convert_to_gray_scale, init_serial, write_screen_buffer};
use crossbeam_channel::bounded;
use crossbeam_channel::{Receiver, Sender};
use lazy_static::lazy_static;
use rdev::{grab, Event, EventType, Key};
use rusttype::Font;
use std::error::Error;
use std::ffi::CString;
use std::fmt;
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

#[derive(Debug)]
struct SuperError {
    side: SuperErrorSideKick,
}

impl fmt::Display for SuperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SuperError is here!")
    }
}

impl Error for SuperError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.side)
    }
}

#[derive(Debug)]
struct SuperErrorSideKick;

impl fmt::Display for SuperErrorSideKick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "App already opened!")
    }
}

impl Error for SuperErrorSideKick {}

fn get_super_error() -> SuperError {
    SuperError {
        side: SuperErrorSideKick,
    }
}

lazy_static! {
    static ref LAST_KEY: Mutex<bool> = Mutex::new(false);
    static ref LAST_KEY_VALUE: Mutex<u32> = Mutex::new(0);
    static ref SERIAL_PORT: Mutex<Option<Box<dyn serialport::SerialPort>>> = Mutex::new(None);
}
pub fn main() -> iced::Result {
    unsafe {
        let app_image = ::image::load_from_memory(include_bytes!("../icon.ico") as &[u8]);

        let lp_text = CString::new("AwesomeInfoDisplay").unwrap();
        winapi::um::synchapi::CreateMutexA(std::ptr::null_mut(), 1, lp_text.as_ptr());
        if winapi::um::errhandlingapi::GetLastError()
            == winapi::shared::winerror::ERROR_ALREADY_EXISTS
        {
            Err(iced::Error::WindowCreationFailed(Box::new(
                get_super_error(),
            )))
        } else {
            let settings = Settings {
                exit_on_close_request: false,
                window: window::Settings {
                    resizable: false,
                    decorations: true,
                    icon: Some(
                        iced::window::icon::Icon::from_rgba(
                            app_image.unwrap().to_rgba8().to_vec(),
                            256,
                            256,
                        )
                        .unwrap(),
                    ),
                    ..Default::default()
                },
                ..Default::default()
            };
            AwesomeDisplay::run(settings)
        }
    }
}

struct AwesomeDisplay {
    theme: style::Theme,
    next_screen: button::State,
    previous_screen: button::State,
    save_config_button: button::State,
    screens: screen_manager::ScreenManager,
    config_manager: Arc<RwLock<config_manager::ConfigManager>>,
    should_exit: bool,
    bitpanda_api_key_input: text_input::State,
    openweather_api_key_input: text_input::State,
    openweather_location_input: text_input::State,
    slider: slider::State,
    sender: Sender<Vec<u8>>,
}

#[derive(Debug, Clone)]
enum Message {
    NextScreen,
    PreviousScreen,
    UpdateCurrentScreen,
    SaveConfig,
    SliderChanged(f32),
    ScreenStatusChanged(bool, String),
    KeyboardEventOccurred(iced::keyboard::KeyCode, u32),
    WindowEventOccurred(iced_native::Event),
    BitpandaApiKeyChanged(String),
    OpenWeatherApiKeyChanged(String),
    OpenWeatherLocationChanged(String),
}

impl Application for AwesomeDisplay {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = ();
    fn new(_flags: ()) -> (AwesomeDisplay, Command<Message>) {
        let font = Rc::new(
            Font::try_from_vec(Vec::from(include_bytes!("Liberation.ttf") as &[u8])).unwrap(),
        );

        let config_manager =
            std::sync::Arc::new(RwLock::new(config_manager::ConfigManager::new(None)));
        let mut screens: Vec<Box<dyn screens::BasicScreen>> = Vec::new();

        screens.push(Box::new(
            screens::system_info_screen::SystemInfoScreen::new(
                String::from("System Info"),
                String::from("system_info_screen"),
                Rc::clone(&font),
                Arc::clone(&config_manager),
            ),
        ));
        screens.push(Box::new(screens::media_info_screen::MediaInfoScreen::new(
            String::from("Media Info"),
            String::from("media_info_screen"),
            Rc::clone(&font),
            Arc::clone(&config_manager),
        )));
        screens.push(Box::new(screens::bitpanda_screen::BitpandaScreen::new(
            String::from("Bitpanda Info"),
            String::from("bitpanda_screen"),
            Rc::clone(&font),
            Arc::clone(&config_manager),
        )));
        screens.push(Box::new(screens::weather_screen::WeatherScreen::new(
            String::from("Weather Info"),
            String::from("weather_screen"),
            Rc::clone(&font),
            Arc::clone(&config_manager),
        )));
        let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = bounded(1);
        let this = AwesomeDisplay {
            next_screen: button::State::new(),
            previous_screen: button::State::new(),
            save_config_button: button::State::new(),
            theme: style::Theme::Dark,
            screens: screen_manager::ScreenManager::new(screens),
            config_manager: config_manager.clone(),
            should_exit: false,
            bitpanda_api_key_input: text_input::State::new(),
            openweather_api_key_input: text_input::State::new(),
            openweather_location_input: text_input::State::new(),
            slider: slider::State::new(),
            sender: tx,
        };

        // global key press listener
        thread::spawn({
            move || loop {
                if let Err(error) = grab(callback) {
                    println!("Error: {:?}", error)
                }
            }
        });

        // write to serial port ... since it is blocking, we'll just do this in a different thread
        thread::spawn(move || loop {
            let buf = rx.recv();
            if SERIAL_PORT.lock().unwrap().is_none() {
                *SERIAL_PORT.lock().unwrap() = init_serial();
            } else {
                match buf {
                    Ok(b) => {
                        if !write_screen_buffer(&mut *SERIAL_PORT.lock().unwrap(), &b) {
                            *SERIAL_PORT.lock().unwrap() = None;
                        }
                    }
                    Err(_) => {}
                }
            }
        });
        (this, Command::none())
    }
    fn title(&self) -> String {
        String::from("AwesomeInfoDisplay")
    }

    fn should_exit(&self) -> bool {
        self.should_exit
    }

    fn subscription(&self) -> Subscription<Message> {
        iced_futures::subscription::Subscription::batch(
            vec![
                iced_native::subscription::events_with(|event, status| {
                    if let iced_native::event::Status::Captured = status {
                        return None;
                    }

                    match event {
                        iced_native::Event::Keyboard(iced::keyboard::Event::KeyReleased {
                            modifiers: _,
                            key_code,
                        }) => match key_code {
                            iced::keyboard::KeyCode::PlayPause => {
                                Some(Message::KeyboardEventOccurred(key_code, 179))
                            }
                            iced::keyboard::KeyCode::MediaStop => {
                                Some(Message::KeyboardEventOccurred(key_code, 178))
                            }
                            iced::keyboard::KeyCode::PrevTrack => {
                                Some(Message::KeyboardEventOccurred(key_code, 177))
                            }
                            iced::keyboard::KeyCode::NextTrack => {
                                Some(Message::KeyboardEventOccurred(key_code, 176))
                            }
                            iced::keyboard::KeyCode::VolumeDown => {
                                Some(Message::KeyboardEventOccurred(key_code, 174))
                            }
                            iced::keyboard::KeyCode::VolumeUp => {
                                Some(Message::KeyboardEventOccurred(key_code, 175))
                            }
                            iced::keyboard::KeyCode::Mute => {
                                Some(Message::KeyboardEventOccurred(key_code, 173))
                            }
                            iced::keyboard::KeyCode::Pause => {
                                Some(Message::KeyboardEventOccurred(key_code, 180))
                            }
                            _ => None,
                        },
                        _ => None,
                    }
                }),
                time::every(std::time::Duration::from_millis(250))
                    .map(|_| Message::UpdateCurrentScreen),
                iced_native::subscription::events().map(Message::WindowEventOccurred),
            ]
            .into_iter(),
        )
    }

    fn update(&mut self, message: Message, _clipboard: &mut iced::Clipboard) -> Command<Message> {
        match message {
            Message::SaveConfig => {
                self.config_manager.write().unwrap().save();
            }
            Message::NextScreen => {
                self.screens.update_current_screen();
                self.screens.next_screen();
                self.screens.update_current_screen();
            }
            Message::PreviousScreen => {
                self.screens.update_current_screen();
                self.screens.previous_screen();
                self.screens.update_current_screen();
            }
            Message::UpdateCurrentScreen => {
                if *LAST_KEY.lock().unwrap() {
                    *LAST_KEY.lock().unwrap() = false;
                    let val = *LAST_KEY_VALUE.lock().unwrap();
                    if val == 174 || val == 175 {
                        // 1 is "volume mode"
                        self.screens
                            .set_screen_for_short("media_info_screen".into(), 1);
                    } else if val >= 176 && val < 180 {
                        // 0 is "normal mode"
                        self.screens
                            .set_screen_for_short("media_info_screen".into(), 0);
                    } else if val == 180 {
                        self.screens.next_screen()
                    }
                    *LAST_KEY_VALUE.lock().unwrap() = 0;
                }
                self.screens.update_current_screen();
            }
            Message::KeyboardEventOccurred(_event, key_code) => {
                // switch to media screen for a few seconds
                *LAST_KEY.lock().unwrap() = true;
                *LAST_KEY_VALUE.lock().unwrap() = key_code;
                self.screens.update_current_screen();
            }
            Message::WindowEventOccurred(event) => {
                if let iced_native::Event::Window(iced_native::window::Event::CloseRequested) =
                    event
                {
                    self.config_manager.write().unwrap().save();
                    self.should_exit = true;
                }
            }
            Message::SliderChanged(slider_value) => {
                self.config_manager.write().unwrap().config.brightness = slider_value as u16;
            }
            Message::ScreenStatusChanged(status, screen) => {
                if self.screens.screen_deactivatable(&screen) {
                    self.screens.set_status_for_screen(&screen, status);
                }
            }
            Message::BitpandaApiKeyChanged(message) => {
                self.config_manager.write().unwrap().config.bitpanda_api_key = message;
            }
            Message::OpenWeatherApiKeyChanged(message) => {
                self.config_manager
                    .write()
                    .unwrap()
                    .config
                    .openweather_api_key = message;
            }
            Message::OpenWeatherLocationChanged(message) => {
                self.config_manager
                    .write()
                    .unwrap()
                    .config
                    .openweather_location = message;
            }
        }
        Command::none()
    }

    fn view(&mut self) -> Element<Message> {
        if !self.screens.current_screen().initial_update_called() {
            self.screens.update_current_screen();
        }
        // RENDER IN APP
        let screen_buffer = self.screens.current_screen().current_image();
        let mut converted_sb = Vec::new();
        for chunk in screen_buffer.chunks(3) {
            converted_sb.push(
                (chunk[2] as f32 * self.config_manager.read().unwrap().config.brightness as f32
                    / 100.0) as u8,
            );
            converted_sb.push(
                (chunk[1] as f32 * self.config_manager.read().unwrap().config.brightness as f32
                    / 100.0) as u8,
            );
            converted_sb.push(
                (chunk[0] as f32 * self.config_manager.read().unwrap().config.brightness as f32
                    / 100.0) as u8,
            );
            converted_sb.push(255);
        }
        let image = Image::new(iced::image::Handle::from_pixels(256, 64, converted_sb));

        // SEND TO DISPLAY
        let bytes = self.screens.current_screen().current_image();
        let bytes = convert_to_gray_scale(bytes);
        match self.sender.try_send(bytes.clone()) {
            Ok(_) => {}
            Err(_) => {}
        }
        let mut column_parts = vec![
            button(
                "Next screen",
                &mut self.next_screen,
                self.theme,
                Message::NextScreen,
            ),
            button(
                "Previous Screen",
                &mut self.previous_screen,
                self.theme,
                Message::PreviousScreen,
            ),
            Text::new(format!(
                "Brightness: {:.2}",
                self.config_manager.read().unwrap().config.brightness
            ))
            .into(),
            Slider::new(
                &mut self.slider,
                0.0..=100.0,
                self.config_manager.read().unwrap().config.brightness as f32,
                Message::SliderChanged,
            )
            .width(Length::Units(200))
            .step(0.1)
            .into(),
        ];

        // insert screens into left column menu
        for screen in self.screens.descriptions_and_keys_and_state().into_iter() {
            column_parts.push(checkbox(screen.2, screen.1.into(), screen.0.into()));
        }
        let mut left_column_after_screens = vec![
            TextInput::new(
                &mut self.bitpanda_api_key_input,
                "Bitpanda Api Key",
                &self.config_manager.read().unwrap().config.bitpanda_api_key,
                Message::BitpandaApiKeyChanged,
            )
            .password()
            .width(Length::Units(200))
            .into(),
            TextInput::new(
                &mut self.openweather_api_key_input,
                "Openweather Api Key",
                &self
                    .config_manager
                    .read()
                    .unwrap()
                    .config
                    .openweather_api_key,
                Message::OpenWeatherApiKeyChanged,
            )
            .password()
            .width(Length::Units(200))
            .into(),
            TextInput::new(
                &mut self.openweather_location_input,
                "Openweather Location",
                &self
                    .config_manager
                    .read()
                    .unwrap()
                    .config
                    .openweather_location,
                Message::OpenWeatherLocationChanged,
            )
            .width(Length::Units(200))
            .into(),
            button(
                "Save config",
                &mut self.save_config_button,
                self.theme,
                Message::SaveConfig,
            ),
        ];
        column_parts.append(&mut left_column_after_screens);
        let col1 = Column::with_children(column_parts)
            .padding(20)
            .align_items(Align::Center)
            .spacing(10);

        let col2 = Column::new()
            .padding(20)
            .align_items(Align::Center)
            .width(Length::Fill)
            .push(Text::new("Current screen").size(50))
            .push(Text::new(self.screens.current_screen().description()).size(25))
            .push(image.width(Length::Units(256)).height(Length::Units(64)));

        Container::new(Row::new().push(col1).push(col2))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(self.theme)
            .into()
    }
}

fn callback(event: Event) -> Option<Event> {
    match event.event_type {
        EventType::KeyPress(Key::Unknown(178)) => {
            *LAST_KEY.lock().unwrap() = true;
            *LAST_KEY_VALUE.lock().unwrap() = 178;
            Some(event)
        }
        EventType::KeyPress(Key::Unknown(177)) => {
            *LAST_KEY.lock().unwrap() = true;
            *LAST_KEY_VALUE.lock().unwrap() = 177;
            Some(event)
        }
        EventType::KeyPress(Key::Unknown(176)) => {
            *LAST_KEY.lock().unwrap() = true;
            *LAST_KEY_VALUE.lock().unwrap() = 176;
            Some(event)
        }
        EventType::KeyPress(Key::Unknown(175)) => {
            *LAST_KEY.lock().unwrap() = true;
            *LAST_KEY_VALUE.lock().unwrap() = 175;
            Some(event)
        }
        EventType::KeyPress(Key::Unknown(174)) => {
            *LAST_KEY.lock().unwrap() = true;
            *LAST_KEY_VALUE.lock().unwrap() = 174;
            Some(event)
        }
        EventType::KeyPress(Key::Unknown(173)) => {
            *LAST_KEY.lock().unwrap() = true;
            *LAST_KEY_VALUE.lock().unwrap() = 173;
            Some(event)
        }
        EventType::KeyPress(Key::Unknown(179)) => {
            *LAST_KEY.lock().unwrap() = true;
            *LAST_KEY_VALUE.lock().unwrap() = 179;
            Some(event)
        }
        EventType::KeyPress(Key::Pause) => {
            *LAST_KEY.lock().unwrap() = true;
            *LAST_KEY_VALUE.lock().unwrap() = 180;
            Some(event)
        }
        _ => Some(event),
    }
}

fn checkbox<'a>(checked: bool, key: String, description: String) -> Element<'a, Message> {
    Checkbox::new(checked, description, move |value: bool| {
        Message::ScreenStatusChanged(value, key.clone())
    })
    .width(Length::Units(200))
    .into()
}

fn button<'a>(
    label: &str,
    button_state: &'a mut button::State,
    theme: style::Theme,
    msg: Message,
) -> Element<'a, Message> {
    Button::new(
        button_state,
        Text::new(label).horizontal_alignment(HorizontalAlignment::Center),
    )
    .style(theme)
    .width(Length::Units(200))
    .on_press(msg)
    .into()
}
