//#![windows_subsystem = "windows"]
use iced::{
    button, executor, time, Align, Application, Button, Column, Command, Container, Element,
    HorizontalAlignment, Image, Length, Row, Settings, Subscription, Text,
};
use std::io::{self, Write};
mod media_info_screen;
mod screen;
mod screen_manager;
mod style;
mod system_info_screen;
use rusttype::Font;
pub fn main() -> iced::Result {
    AwesomeDisplay::run(Settings::default())
}

#[derive(Default)]
struct AwesomeDisplay {
    theme: style::Theme,
    increment_button: button::State,
    decrement_button: button::State,
    screens: screen_manager::ScreenManager,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    NextScreen,
    PreviousScreen,
    UpdateCurrentScreen,
}

impl Application for AwesomeDisplay {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = ();
    fn new(_flags: ()) -> (AwesomeDisplay, Command<Message>) {
        let font = Font::try_from_vec(Vec::from(include_bytes!("Liberation.ttf") as &[u8]));

        (
            AwesomeDisplay {
                increment_button: button::State::new(),
                decrement_button: button::State::new(),
                theme: style::Theme::Dark,
                screens: screen_manager::ScreenManager::new(vec![
                    Box::new(system_info_screen::SystemInfoScreen::new(
                        String::from("System Stats"),
                        font.clone(),
                    )),
                    Box::new(media_info_screen::MediaInfoScreen::new(
                        String::from("Media Stats"),
                        font.clone(),
                    )),
                ]),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("AwesomeInfoDisplay")
    }

    fn subscription(&self) -> Subscription<Message> {
        time::every(std::time::Duration::from_millis(500)).map(|_| Message::UpdateCurrentScreen)
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::NextScreen => {
                self.screens.update_current_screen();
                self.screens.next_screen();
            }
            Message::PreviousScreen => {
                self.screens.update_current_screen();
                self.screens.previous_screen();
            }
            Message::UpdateCurrentScreen => {
                self.screens.update_current_screen();
            }
        }
        Command::none()
    }

    fn view(&mut self) -> Element<Message> {
        io::stdout().flush().unwrap();
        self.screens.update_current_screen();

        let image = Image::new(iced::image::Handle::from_memory(
            self.screens.current_screen().current_image(),
        ));

        let col1 = Column::new()
            .padding(20)
            .align_items(Align::Center)
            .spacing(10)
            .push(
                Button::new(
                    &mut self.increment_button,
                    Text::new("Next screen").horizontal_alignment(HorizontalAlignment::Center),
                )
                .style(self.theme)
                .width(Length::Units(200))
                .on_press(Message::NextScreen),
            )
            .push(
                Button::new(
                    &mut self.decrement_button,
                    Text::new("Previous screen").horizontal_alignment(HorizontalAlignment::Center),
                )
                .style(self.theme)
                .width(Length::Units(200))
                .on_press(Message::PreviousScreen),
            );

        let col2 = Column::new()
            .padding(20)
            .align_items(Align::Center)
            .width(Length::Fill)
            .push(Text::new("Current screen").size(50))
            .push(Text::new(self.screens.current_screen().description()).size(50))
            .push(image.width(Length::Units(256)).height(Length::Units(64)));

        Container::new(Row::new().push(col1).push(col2))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(self.theme)
            .into()
    }
}