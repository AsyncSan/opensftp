mod core;
mod theme;
mod ui;

use iced::{application, Font};

fn main() -> iced::Result {
    application("EuroSFTP", ui::app::App::update, ui::app::App::view)
        .theme(ui::app::App::theme)
        .subscription(ui::app::App::subscription)
        .window_size(iced::Size::new(1200.0, 750.0))
        .default_font(Font::MONOSPACE)
        .run()
}
