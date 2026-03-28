use iced::widget::{column, container, row, scrollable, text, button, horizontal_space};
use iced::{Element, Length, Alignment};

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: String,
}

impl FileEntry {
    pub fn size_display(&self) -> String {
        if self.is_dir {
            return "DIR".to_string();
        }
        let units = ["B", "KB", "MB", "GB"];
        let mut size = self.size as f64;
        for unit in &units {
            if size < 1024.0 {
                return format!("{:.1} {}", size, unit);
            }
            size /= 1024.0;
        }
        format!("{:.1} TB", size)
    }
}

#[derive(Debug, Clone)]
pub enum PanelMessage {
    EntryClicked(usize),
    GoUp,
    Refresh,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_display_dir() {
        let entry = FileEntry {
            name: "docs".into(),
            is_dir: true,
            size: 4096,
            modified: String::new(),
        };
        assert_eq!(entry.size_display(), "DIR");
    }

    #[test]
    fn size_display_bytes() {
        let entry = FileEntry {
            name: "small.txt".into(),
            is_dir: false,
            size: 512,
            modified: String::new(),
        };
        assert_eq!(entry.size_display(), "512.0 B");
    }

    #[test]
    fn size_display_kilobytes() {
        let entry = FileEntry {
            name: "medium.txt".into(),
            is_dir: false,
            size: 2048,
            modified: String::new(),
        };
        assert_eq!(entry.size_display(), "2.0 KB");
    }

    #[test]
    fn size_display_megabytes() {
        let entry = FileEntry {
            name: "large.bin".into(),
            is_dir: false,
            size: 5 * 1024 * 1024,
            modified: String::new(),
        };
        assert_eq!(entry.size_display(), "5.0 MB");
    }

    #[test]
    fn size_display_gigabytes() {
        let entry = FileEntry {
            name: "huge.iso".into(),
            is_dir: false,
            size: 2 * 1024 * 1024 * 1024,
            modified: String::new(),
        };
        assert_eq!(entry.size_display(), "2.0 GB");
    }

    #[test]
    fn size_display_terabytes() {
        let entry = FileEntry {
            name: "massive.img".into(),
            is_dir: false,
            size: 5 * 1024 * 1024 * 1024 * 1024,
            modified: String::new(),
        };
        assert_eq!(entry.size_display(), "5.0 TB");
    }

    #[test]
    fn size_display_zero_bytes() {
        let entry = FileEntry {
            name: "empty.txt".into(),
            is_dir: false,
            size: 0,
            modified: String::new(),
        };
        assert_eq!(entry.size_display(), "0.0 B");
    }

    #[test]
    fn size_display_boundary_1024() {
        let entry = FileEntry {
            name: "exact.txt".into(),
            is_dir: false,
            size: 1024,
            modified: String::new(),
        };
        assert_eq!(entry.size_display(), "1.0 KB");
    }
}

pub fn view_panel<'a>(
    title: &'a str,
    path: &'a str,
    entries: &'a [FileEntry],
    selected: Option<usize>,
) -> Element<'a, PanelMessage> {
    let header = row![
        button("<").on_press(PanelMessage::GoUp).padding(4),
        text(path).size(13),
        horizontal_space(),
        button("Refresh").on_press(PanelMessage::Refresh).padding(4),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let col_header = row![
        text("Name").size(12).width(Length::FillPortion(4)),
        text("Size").size(12).width(Length::FillPortion(1)),
        text("Modified").size(12).width(Length::FillPortion(2)),
    ]
    .spacing(4);

    let file_rows: Vec<Element<'a, PanelMessage>> = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let icon = if entry.is_dir { "📁" } else { "📄" };
            let r = button(
                row![
                    text(format!("{} {}", icon, entry.name))
                        .size(13)
                        .width(Length::FillPortion(4)),
                    text(entry.size_display())
                        .size(13)
                        .width(Length::FillPortion(1)),
                    text(&entry.modified)
                        .size(13)
                        .width(Length::FillPortion(2)),
                ]
                .spacing(4),
            )
            .on_press(PanelMessage::EntryClicked(i))
            .padding(4)
            .width(Length::Fill);

            if selected == Some(i) {
                container(r)
                    .width(Length::Fill)
                    .into()
            } else {
                container(r)
                    .width(Length::Fill)
                    .into()
            }
        })
        .collect();

    let list = scrollable(
        column(file_rows).spacing(1),
    )
    .height(Length::Fill);

    container(
        column![
            text(title).size(16),
            header,
            col_header,
            list,
        ]
        .spacing(8)
        .padding(12),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
