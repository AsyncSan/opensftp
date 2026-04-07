use iced::widget::{column, container, mouse_area, row, scrollable, text, text_input, button, horizontal_space};
use iced::{Element, Length, Alignment, Theme};
use super::icons;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: String,
    pub permissions: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Name,
    Size,
    Modified,
    Permissions,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PanelMessage {
    EntryClicked(usize, bool, bool), // (index, ctrl_held, shift_held)
    GoUp,
    Refresh,
    PathSegmentClicked(String),
    EntryRightClicked(usize),
    FilterChanged(String),
    SortBy(SortColumn),
    GoHome,
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
            permissions: String::new(),
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
            permissions: String::new(),
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
            permissions: String::new(),
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
            permissions: String::new(),
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
            permissions: String::new(),
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
            permissions: String::new(),
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
            permissions: String::new(),
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
            permissions: String::new(),
        };
        assert_eq!(entry.size_display(), "1.0 KB");
    }
}

pub fn view_panel<'a>(
    title: &'a str,
    path: &'a str,
    entries: &'a [FileEntry],
    selected: &'a std::collections::BTreeSet<usize>,
    filter: &'a str,
    is_active: bool,
    sort_column: SortColumn,
    sort_ascending: bool,
) -> Element<'a, PanelMessage> {
    // Clickable breadcrumbs
    let mut breadcrumbs = row![
        button(icons::arrow_left(12.0)).on_press(PanelMessage::GoUp).padding([2, 6]),
        button(icons::home(12.0)).on_press(PanelMessage::GoHome).padding([2, 6]),
    ]
    .spacing(1)
    .align_y(Alignment::Center);

    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    // Root
    breadcrumbs = breadcrumbs.push(
        button(text("/").size(12))
            .on_press(PanelMessage::PathSegmentClicked("/".to_string()))
            .padding([2, 3]),
    );
    for (i, seg) in segments.iter().enumerate() {
        let full_path = format!("/{}", segments[..=i].join("/"));
        breadcrumbs = breadcrumbs.push(
            button(text(format!("{}/", seg)).size(12))
                .on_press(PanelMessage::PathSegmentClicked(full_path))
                .padding([2, 3]),
        );
    }
    breadcrumbs = breadcrumbs.push(horizontal_space());
    breadcrumbs = breadcrumbs.push(
        button(icons::refresh(12.0)).on_press(PanelMessage::Refresh).padding([2, 6]),
    );

    let header = breadcrumbs;

    let filter_input = text_input("Filter...", filter)
        .on_input(PanelMessage::FilterChanged)
        .padding(4)
        .size(12)
        .width(Length::Fill);

    let sort_arrow = |col: SortColumn| -> &str {
        if sort_column == col {
            if sort_ascending { " ^" } else { " v" }
        } else {
            ""
        }
    };

    let col_header = row![
        button(text(format!("Name{}", sort_arrow(SortColumn::Name))).size(12))
            .on_press(PanelMessage::SortBy(SortColumn::Name))
            .padding(2)
            .width(Length::FillPortion(4)),
        button(text(format!("Perms{}", sort_arrow(SortColumn::Permissions))).size(12))
            .on_press(PanelMessage::SortBy(SortColumn::Permissions))
            .padding(2)
            .width(Length::FillPortion(1)),
        button(text(format!("Size{}", sort_arrow(SortColumn::Size))).size(12))
            .on_press(PanelMessage::SortBy(SortColumn::Size))
            .padding(2)
            .width(Length::FillPortion(1)),
        button(text(format!("Modified{}", sort_arrow(SortColumn::Modified))).size(12))
            .on_press(PanelMessage::SortBy(SortColumn::Modified))
            .padding(2)
            .width(Length::FillPortion(2)),
    ]
    .spacing(4);

    let filter_lower = filter.to_lowercase();
    let file_rows: Vec<Element<'a, PanelMessage>> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            entry.name == ".." || filter_lower.is_empty() || entry.name.to_lowercase().contains(&filter_lower)
        })
        .enumerate()
        .map(|(visual_idx, (i, entry))| {
            let icon_widget = if entry.is_dir { icons::folder(12.0) } else { icons::file(12.0) };
            let r = button(
                row![
                    row![
                        icon_widget,
                        text(&entry.name).size(12),
                    ].spacing(4).width(Length::FillPortion(4)),
                    text(&entry.permissions)
                        .size(11)
                        .width(Length::FillPortion(1)),
                    text(entry.size_display())
                        .size(12)
                        .width(Length::FillPortion(1)),
                    text(&entry.modified)
                        .size(11)
                        .width(Length::FillPortion(2)),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .on_press(PanelMessage::EntryClicked(i, false, false))
            .padding([3, 6])
            .width(Length::Fill);

            let is_selected = selected.contains(&i);
            let is_even = visual_idx % 2 == 0;
            let c = container(r).width(Length::Fill);
            let styled: Element<'a, PanelMessage> = if is_selected {
                c.style(|theme: &Theme| {
                    container::Style {
                        background: Some(iced::Background::Color(
                            theme.extended_palette().primary.weak.color,
                        )),
                        ..Default::default()
                    }
                })
                .into()
            } else if is_even {
                c.style(|theme: &Theme| {
                    let bg = theme.extended_palette().background.weak.color;
                    container::Style {
                        background: Some(iced::Background::Color(iced::Color {
                            a: 0.3,
                            ..bg
                        })),
                        ..Default::default()
                    }
                })
                .into()
            } else {
                c.into()
            };
            mouse_area(styled)
                .on_right_press(PanelMessage::EntryRightClicked(i))
                .into()
        })
        .collect();

    let list = scrollable(
        column(file_rows).spacing(1),
    )
    .height(Length::Fill);

    let panel = container(
        column![
            text(title).size(14),
            header,
            filter_input,
            col_header,
            list,
        ]
        .spacing(6)
        .padding(12),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    if is_active {
        panel.style(|theme: &Theme| {
            container::Style {
                border: iced::Border {
                    color: theme.extended_palette().primary.strong.color,
                    width: 2.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
    } else {
        panel.style(|theme: &Theme| {
            container::Style {
                border: iced::Border {
                    color: theme.extended_palette().background.strong.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
    }
}
