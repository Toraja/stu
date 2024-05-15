use chrono::{DateTime, Local};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph, Tabs, Wrap},
    Frame,
};

use crate::{
    app::DetailViewState,
    component::AppListState,
    object::{FileDetail, FileVersion, ObjectItem},
    pages::page::Page,
    util::digits,
};

const SELECTED_COLOR: Color = Color::Cyan;
const SELECTED_ITEM_TEXT_COLOR: Color = Color::Black;
const SELECTED_DISABLED_COLOR: Color = Color::DarkGray;
const DIVIDER_COLOR: Color = Color::DarkGray;

pub struct ObjectDetailPage {
    object_items: Vec<ObjectItem>,
    file_detail: FileDetail,
    file_versions: Vec<FileVersion>,

    vs: DetailViewState,
    list_state: AppListState,
}

impl ObjectDetailPage {
    pub fn new(
        object_items: Vec<ObjectItem>,
        file_detail: FileDetail,
        file_versions: Vec<FileVersion>,
        vs: DetailViewState,
        height: usize,
    ) -> Self {
        Self {
            object_items,
            file_detail,
            file_versions,
            vs,
            list_state: AppListState::new(height),
        }
    }
}

impl Page for ObjectDetailPage {
    fn render(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::horizontal(Constraint::from_percentages([50, 50])).split(area);

        let list_state = ListViewState {
            current_selected: self.list_state.selected,
            current_offset: self.list_state.offset,
        };
        let styles = ListItemStyles {
            selected_bg_color: SELECTED_DISABLED_COLOR,
            selected_fg_color: SELECTED_ITEM_TEXT_COLOR,
        };
        let list_items = build_list_items_from_object_items(
            &self.object_items,
            list_state,
            chunks[0],
            styles,
            false,
        );
        let list = build_list(
            list_items,
            self.object_items.len(),
            list_state.current_selected,
        );
        f.render_widget(list, chunks[0]);

        let block = build_file_detail_block();
        f.render_widget(block, chunks[1]);

        let chunks = Layout::vertical([Constraint::Length(2), Constraint::Min(0)])
            .margin(1)
            .split(chunks[1]);

        let tabs = build_file_detail_tabs(&self.vs);
        f.render_widget(tabs, chunks[0]);

        match self.vs {
            DetailViewState::Detail => {
                let detail = build_file_detail(&self.file_detail);
                f.render_widget(detail, chunks[1]);
            }
            DetailViewState::Version => {
                let versions = build_file_versions(&self.file_versions, chunks[1].width);
                f.render_widget(versions, chunks[1]);
            }
        }
    }
}

fn build_list_items_from_object_items(
    current_items: &[ObjectItem],
    list_state: ListViewState,
    area: Rect,
    styles: ListItemStyles,
    show_file_detail: bool,
) -> Vec<ListItem> {
    let show_item_count = (area.height as usize) - 2 /* border */;
    current_items
        .iter()
        .skip(list_state.current_offset)
        .take(show_item_count)
        .enumerate()
        .map(|(idx, item)| {
            build_list_item_from_object_item(idx, item, list_state, area, styles, show_file_detail)
        })
        .collect()
}

fn build_list_item_from_object_item(
    idx: usize,
    item: &ObjectItem,
    list_state: ListViewState,
    area: Rect,
    styles: ListItemStyles,
    show_file_detail: bool,
) -> ListItem {
    let content = match item {
        ObjectItem::Dir { name, .. } => {
            let content = format_dir_item(name, area.width);
            let style = Style::default().add_modifier(Modifier::BOLD);
            Span::styled(content, style)
        }
        ObjectItem::File {
            name,
            size_byte,
            last_modified,
            ..
        } => {
            let content = format_file_item(
                name,
                *size_byte,
                last_modified,
                area.width,
                show_file_detail,
            );
            let style = Style::default();
            Span::styled(content, style)
        }
    };
    if idx + list_state.current_offset == list_state.current_selected {
        ListItem::new(content).style(
            Style::default()
                .bg(styles.selected_bg_color)
                .fg(styles.selected_fg_color),
        )
    } else {
        ListItem::new(content)
    }
}

fn format_dir_item(name: &str, width: u16) -> String {
    let name_w: usize = (width as usize) - 2 /* spaces */ - 2 /* border */;
    let name = format!("{}/", name);
    format!(" {:<name_w$} ", name, name_w = name_w)
}

fn format_file_item(
    name: &str,
    size_byte: usize,
    last_modified: &DateTime<Local>,
    width: u16,
    show_file_detail: bool,
) -> String {
    if show_file_detail {
        let size = format_size_byte(size_byte);
        let date = format_datetime(last_modified);
        let date_w: usize = 19;
        let size_w: usize = 10;
        let name_w: usize = (width as usize) - date_w - size_w - 10 /* spaces */ - 4 /* border + space */;
        format!(
            " {:<name_w$}    {:<date_w$}    {:>size_w$} ",
            name,
            date,
            size,
            name_w = name_w,
            date_w = date_w,
            size_w = size_w
        )
    } else {
        let name_w: usize = (width as usize) - 2 /* spaces */ - 4 /* border */;
        format!(" {:<name_w$} ", name, name_w = name_w)
    }
}

fn build_list(list_items: Vec<ListItem>, total_count: usize, current_selected: usize) -> List {
    let title = format_list_count(total_count, current_selected);
    List::new(list_items).block(
        Block::bordered()
            .title(title)
            .title_alignment(Alignment::Right)
            .padding(Padding::horizontal(1)),
    )
}

fn build_file_detail_block() -> Block<'static> {
    Block::bordered()
}

fn build_file_detail_tabs(selected: &DetailViewState) -> Tabs {
    let tabs = vec![Line::from("Detail"), Line::from("Version")];
    Tabs::new(tabs)
        .select(*selected as usize)
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(SELECTED_COLOR),
        )
        .block(Block::default().borders(Borders::BOTTOM))
}

fn build_file_detail(detail: &FileDetail) -> Paragraph {
    let details = [
        ("Name:", &detail.name),
        ("Size:", &format_size_byte(detail.size_byte)),
        ("Last Modified:", &format_datetime(&detail.last_modified)),
        ("ETag:", &detail.e_tag),
        ("Content-Type:", &detail.content_type),
        ("Storage class:", &detail.storage_class),
    ]
    .iter()
    .filter_map(|(label, value)| {
        if value.is_empty() {
            None
        } else {
            let lines = vec![
                Line::from(label.add_modifier(Modifier::BOLD)),
                Line::from(format!(" {}", value)),
            ];
            Some(lines)
        }
    })
    .collect();

    let content = flatten_with_empty_lines(details, false);
    Paragraph::new(content)
        .block(Block::default().padding(Padding::horizontal(1)))
        .wrap(Wrap { trim: false })
}

fn build_file_versions(versions: &[FileVersion], width: u16) -> List {
    let list_items: Vec<ListItem> = versions
        .iter()
        .map(|v| {
            let content = vec![
                Line::from(vec![
                    "    Version ID: ".add_modifier(Modifier::BOLD),
                    Span::raw(format_version(&v.version_id)),
                ]),
                Line::from(vec![
                    " Last Modified: ".add_modifier(Modifier::BOLD),
                    Span::raw(format_datetime(&v.last_modified)),
                ]),
                Line::from(vec![
                    "          Size: ".add_modifier(Modifier::BOLD),
                    Span::raw(format_size_byte(v.size_byte)),
                ]),
                Line::from("-".repeat(width as usize).fg(DIVIDER_COLOR)),
            ];
            ListItem::new(content)
        })
        .collect();
    List::new(list_items)
        .block(Block::default())
        .highlight_style(Style::default().bg(SELECTED_COLOR))
}

fn format_list_count(total_count: usize, current_selected: usize) -> String {
    if total_count == 0 {
        String::new()
    } else {
        format_count(current_selected + 1, total_count)
    }
}

fn format_count(selected: usize, total: usize) -> String {
    let digits = digits(total);
    format!(" {:>digits$} / {} ", selected, total)
}

fn format_size_byte(size_byte: usize) -> String {
    humansize::format_size_i(size_byte, humansize::BINARY)
}

#[cfg(not(feature = "imggen"))]
fn format_version(version: &str) -> &str {
    version
}

#[cfg(feature = "imggen")]
fn format_version(_version: &str) -> &str {
    "GeJeVLwoQlknMCcSa"
}

#[cfg(not(feature = "imggen"))]
fn format_datetime(datetime: &DateTime<Local>) -> String {
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(feature = "imggen")]
fn format_datetime(_datetime: &DateTime<Local>) -> String {
    String::from("2024-01-02 13:04:05")
}

#[derive(Clone, Copy, Debug)]
struct ListViewState {
    current_selected: usize,
    current_offset: usize,
}

#[derive(Clone, Copy, Debug)]
struct ListItemStyles {
    selected_bg_color: Color,
    selected_fg_color: Color,
}

fn flatten_with_empty_lines(line_groups: Vec<Vec<Line>>, add_to_end: bool) -> Vec<Line> {
    let n = line_groups.len();
    let mut ret: Vec<Line> = Vec::new();
    for (i, lines) in line_groups.into_iter().enumerate() {
        for line in lines {
            ret.push(line);
        }
        if add_to_end || i != n - 1 {
            ret.push(Line::from(""));
        }
    }
    ret
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

    #[test]
    fn test_render_detail_tab() -> std::io::Result<()> {
        let mut terminal = setup_terminal()?;

        terminal.draw(|f| {
            let (items, file_detail, file_versions) = fixtures();
            let mut page = ObjectDetailPage::new(
                items,
                file_detail,
                file_versions,
                DetailViewState::Detail,
                20,
            );
            let area = Rect::new(0, 0, 60, 20);
            page.render(f, area);
        })?;

        #[rustfmt::skip]
        let mut expected = Buffer::with_lines([
            "┌───────────────────── 1 / 3 ┐┌────────────────────────────┐",
            "│  file1                     ││ Detail │ Version           │",
            "│  file2                     ││────────────────────────────│",
            "│  file3                     ││ Name:                      │",
            "│                            ││  file1                     │",
            "│                            ││                            │",
            "│                            ││ Size:                      │",
            "│                            ││  1.01 KiB                  │",
            "│                            ││                            │",
            "│                            ││ Last Modified:             │",
            "│                            ││  2024-01-02 13:01:02       │",
            "│                            ││                            │",
            "│                            ││ ETag:                      │",
            "│                            ││  bef684de-a260-48a4-8178-8 │",
            "│                            ││ a535ecccadb                │",
            "│                            ││                            │",
            "│                            ││ Content-Type:              │",
            "│                            ││  text/plain                │",
            "│                            ││                            │",
            "└────────────────────────────┘└────────────────────────────┘",
        ]);
        for x in 2..28 {
            // selected item
            expected.get_mut(x, 1).set_bg(Color::DarkGray);
            expected.get_mut(x, 1).set_fg(Color::Black);
        }
        for x in 32..38 {
            // "Detail" is selected
            expected
                .get_mut(x, 1)
                .set_fg(Color::Cyan)
                .set_style(Style::default().add_modifier(Modifier::BOLD));
        }
        for x in 32..37 {
            // "Name" label
            expected
                .get_mut(x, 3)
                .set_style(Style::default().add_modifier(Modifier::BOLD));
        }
        for x in 32..37 {
            // "Size" label
            expected
                .get_mut(x, 6)
                .set_style(Style::default().add_modifier(Modifier::BOLD));
        }
        for x in 32..46 {
            // "Last Modified" label
            expected
                .get_mut(x, 9)
                .set_style(Style::default().add_modifier(Modifier::BOLD));
        }
        for x in 32..37 {
            // "ETag" label
            expected
                .get_mut(x, 12)
                .set_style(Style::default().add_modifier(Modifier::BOLD));
        }
        for x in 32..45 {
            // "Content-Type" label
            expected
                .get_mut(x, 16)
                .set_style(Style::default().add_modifier(Modifier::BOLD));
        }

        terminal.backend().assert_buffer(&expected);

        Ok(())
    }

    #[test]
    fn test_render_version_tab() -> std::io::Result<()> {
        let mut terminal = setup_terminal()?;

        terminal.draw(|f| {
            let (items, file_detail, file_versions) = fixtures();
            let mut page = ObjectDetailPage::new(
                items,
                file_detail,
                file_versions,
                DetailViewState::Version,
                20,
            );
            let area = Rect::new(0, 0, 60, 20);
            page.render(f, area);
        })?;

        #[rustfmt::skip]
        let mut expected = Buffer::with_lines([
            "┌───────────────────── 1 / 3 ┐┌────────────────────────────┐",
            "│  file1                     ││ Detail │ Version           │",
            "│  file2                     ││────────────────────────────│",
            "│  file3                     ││    Version ID: 60f36bc2-0f3│",
            "│                            ││ Last Modified: 2024-01-02 1│",
            "│                            ││          Size: 1.01 KiB    │",
            "│                            ││----------------------------│",
            "│                            ││    Version ID: 1c5d3bcc-2bb│",
            "│                            ││ Last Modified: 2024-01-01 2│",
            "│                            ││          Size: 1 KiB       │",
            "│                            ││----------------------------│",
            "│                            ││                            │",
            "│                            ││                            │",
            "│                            ││                            │",
            "│                            ││                            │",
            "│                            ││                            │",
            "│                            ││                            │",
            "│                            ││                            │",
            "│                            ││                            │",
            "└────────────────────────────┘└────────────────────────────┘",
        ]);
        for x in 2..28 {
            // selected item
            expected.get_mut(x, 1).set_bg(Color::DarkGray);
            expected.get_mut(x, 1).set_fg(Color::Black);
        }
        for x in 41..48 {
            // "Version" is selected
            expected
                .get_mut(x, 1)
                .set_fg(Color::Cyan)
                .set_style(Style::default().add_modifier(Modifier::BOLD));
        }
        for x in 31..47 {
            for y in [3, 7].into_iter() {
                // "Version ID" label
                expected
                    .get_mut(x, y)
                    .set_style(Style::default().add_modifier(Modifier::BOLD));
            }
        }
        for x in 31..47 {
            for y in [4, 8].into_iter() {
                // "Last Modified" label
                expected
                    .get_mut(x, y)
                    .set_style(Style::default().add_modifier(Modifier::BOLD));
            }
        }
        for x in 31..47 {
            for y in [5, 9].into_iter() {
                // "Size" label
                expected
                    .get_mut(x, y)
                    .set_style(Style::default().add_modifier(Modifier::BOLD));
            }
        }
        for x in 31..59 {
            for y in [6, 10].into_iter() {
                // divider
                expected.get_mut(x, y).set_fg(Color::DarkGray);
            }
        }

        terminal.backend().assert_buffer(&expected);

        Ok(())
    }

    fn setup_terminal() -> std::io::Result<Terminal<TestBackend>> {
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;
        Ok(terminal)
    }

    fn parse_datetime(s: &str) -> DateTime<Local> {
        DateTime::parse_from_rfc3339(s)
            .unwrap()
            .with_timezone(&Local)
    }

    fn fixtures() -> (Vec<ObjectItem>, FileDetail, Vec<FileVersion>) {
        let items = vec![
            ObjectItem::File {
                name: "file1".to_string(),
                size_byte: 1024 + 10,
                last_modified: parse_datetime("2024-01-02T13:01:02+09:00"),
                paths: vec![],
            },
            ObjectItem::File {
                name: "file2".to_string(),
                size_byte: 1024 * 999,
                last_modified: parse_datetime("2023-12-31T09:00:00+09:00"),
                paths: vec![],
            },
            ObjectItem::File {
                name: "file3".to_string(),
                size_byte: 1024,
                last_modified: parse_datetime("2024-01-03T12:59:59+09:00"),
                paths: vec![],
            },
        ];
        let file_detail = FileDetail {
            name: "file1".to_string(),
            size_byte: 1024 + 10,
            last_modified: parse_datetime("2024-01-02T13:01:02+09:00"),
            e_tag: "bef684de-a260-48a4-8178-8a535ecccadb".to_string(),
            content_type: "text/plain".to_string(),
            storage_class: "STANDARD".to_string(),
            key: "file1".to_string(),
            s3_uri: "s3://bucket-1/file1".to_string(),
            arn: "arn:aws:s3:::bucket-1/file1".to_string(),
            object_url: "https://bucket-1.s3.ap-northeast-1.amazonaws.com/file1".to_string(),
        };
        let file_versions = vec![
            FileVersion {
                version_id: "60f36bc2-0f38-47b8-9bf0-e24e334b86d5".to_string(),
                size_byte: 1024 + 10,
                last_modified: parse_datetime("2024-01-02T13:01:02+09:00"),
                is_latest: true,
            },
            FileVersion {
                version_id: "1c5d3bcc-2bb3-4cd5-875f-a95a6ae53f65".to_string(),
                size_byte: 1024,
                last_modified: parse_datetime("2024-01-01T23:59:59+09:00"),
                is_latest: false,
            },
        ];
        (items, file_detail, file_versions)
    }
}