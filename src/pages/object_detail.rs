use crossterm::event::{KeyCode, KeyEvent};
use itsuki::zero_indexed_enum;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, ListItem, Padding, Paragraph, StatefulWidget, Tabs, Widget},
    Frame,
};

use crate::{
    event::{AppEventType, Sender},
    key_code, key_code_char,
    object::{FileDetail, FileVersion, ObjectItem},
    pages::util::{build_helps, build_short_helps},
    ui::common::{format_datetime, format_size_byte, format_version},
    widget::{
        Bar, CopyDetailDialog, CopyDetailDialogState, Divider, InputDialog, InputDialogState,
        ScrollLines, ScrollLinesOptions, ScrollLinesState, ScrollList, ScrollListState,
    },
};

const SELECTED_COLOR: Color = Color::Cyan;
const SELECTED_ITEM_TEXT_COLOR: Color = Color::Black;
const SELECTED_DISABLED_COLOR: Color = Color::DarkGray;

#[derive(Debug)]
pub struct ObjectDetailPage {
    file_detail: FileDetail,
    file_versions: Vec<FileVersion>,

    tab: Tab,
    view_state: ViewState,

    object_items: Vec<ObjectItem>,
    list_state: ScrollListState,
    detail_tab_state: DetailTabState,
    version_tab_state: VersionTabState,
    tx: Sender,
}

#[derive(Default)]
#[zero_indexed_enum]
enum Tab {
    #[default]
    Detail,
    Version,
}

#[derive(Debug, Default)]
enum ViewState {
    #[default]
    Default,
    SaveDialog(InputDialogState),
    CopyDetailDialog(CopyDetailDialogState),
}

impl ObjectDetailPage {
    pub fn new(
        file_detail: FileDetail,
        file_versions: Vec<FileVersion>,
        object_items: Vec<ObjectItem>,
        list_state: ScrollListState,
        tx: Sender,
    ) -> Self {
        let detail_tab_state = DetailTabState::new(&file_detail);
        let version_tab_state = VersionTabState::new(&file_versions);
        Self {
            file_detail,
            file_versions,
            tab: Tab::Detail,
            view_state: ViewState::Default,
            object_items,
            list_state,
            detail_tab_state,
            version_tab_state,
            tx,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match self.view_state {
            ViewState::Default => match key {
                key_code_char!('q') => {
                    self.tx.send(AppEventType::Quit);
                }
                key_code!(KeyCode::Esc) => {
                    self.tx.send(AppEventType::CloseCurrentPage);
                }
                key_code_char!('h') => {
                    self.tx.send(AppEventType::CloseCurrentPage);
                }
                key_code!(KeyCode::Tab) | key_code!(KeyCode::BackTab) => {
                    self.toggle_tab();
                }
                key_code_char!('j') => match self.tab {
                    Tab::Detail => {
                        self.detail_tab_state.scroll_lines_state.scroll_forward();
                    }
                    Tab::Version => {
                        self.version_tab_state.select_next();
                    }
                },
                key_code_char!('k') => match self.tab {
                    Tab::Detail => {
                        self.detail_tab_state.scroll_lines_state.scroll_backward();
                    }
                    Tab::Version => {
                        self.version_tab_state.select_prev();
                    }
                },
                key_code_char!('g') => {
                    if self.tab == Tab::Version {
                        self.version_tab_state.select_first();
                    }
                }
                key_code_char!('G') => {
                    if self.tab == Tab::Version {
                        self.version_tab_state.select_last();
                    }
                }
                key_code_char!('s') => {
                    self.download();
                }
                key_code_char!('S') => {
                    self.open_save_dialog();
                }
                key_code_char!('p') | key_code_char!('l') => {
                    self.preview();
                }
                key_code_char!('r') => {
                    self.open_copy_detail_dialog();
                }
                key_code_char!('x') => {
                    self.open_management_console();
                }
                key_code_char!('?') => {
                    self.tx.send(AppEventType::OpenHelp);
                }
                _ => {}
            },
            ViewState::SaveDialog(ref mut state) => match key {
                key_code!(KeyCode::Esc) => {
                    self.close_save_dialog();
                }
                key_code!(KeyCode::Enter) => {
                    let input = state.input().into();
                    self.download_as(input);
                }
                key_code_char!('?') => {
                    self.tx.send(AppEventType::OpenHelp);
                }
                _ => {
                    state.handle_key_event(key);
                }
            },
            ViewState::CopyDetailDialog(ref mut state) => match key {
                key_code!(KeyCode::Esc) | key_code!(KeyCode::Backspace) => {
                    self.close_copy_detail_dialog();
                }
                key_code!(KeyCode::Enter) => {
                    let (name, value) = state.selected_name_and_value(&self.file_detail);
                    self.tx.send(AppEventType::CopyToClipboard(name, value));
                }
                key_code_char!('j') => {
                    state.select_next();
                }
                key_code_char!('k') => {
                    state.select_prev();
                }
                key_code_char!('?') => {
                    self.tx.send(AppEventType::OpenHelp);
                }
                _ => {}
            },
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::horizontal(Constraint::from_percentages([50, 50])).split(area);

        let offset = self.list_state.offset;
        let selected = self.list_state.selected;

        let list_items =
            build_list_items_from_object_items(&self.object_items, offset, selected, chunks[0]);

        let list = ScrollList::new(list_items);
        f.render_stateful_widget(list, chunks[0], &mut self.list_state);

        let block = Block::bordered();
        f.render_widget(block, chunks[1]);

        let chunks = Layout::vertical([Constraint::Length(2), Constraint::Min(0)])
            .margin(1)
            .split(chunks[1]);

        let tabs = build_tabs(self.tab);
        f.render_widget(tabs, chunks[0]);

        match self.tab {
            Tab::Detail => {
                let detail = DetailTab::default();
                f.render_stateful_widget(detail, chunks[1], &mut self.detail_tab_state);
            }
            Tab::Version => {
                let version = VersionTab::default();
                f.render_stateful_widget(version, chunks[1], &mut self.version_tab_state);
            }
        }

        if let ViewState::SaveDialog(state) = &mut self.view_state {
            let save_dialog = InputDialog::default().title("Save As").max_width(40);
            f.render_stateful_widget(save_dialog, area, state);

            let (cursor_x, cursor_y) = state.cursor();
            f.set_cursor(cursor_x, cursor_y);
        }

        if let ViewState::CopyDetailDialog(state) = &self.view_state {
            let copy_detail_dialog = CopyDetailDialog::new(*state, &self.file_detail);
            f.render_widget(copy_detail_dialog, area);
        }
    }

    pub fn helps(&self) -> Vec<String> {
        let helps: &[(&[&str], &str)] = match self.view_state {
            ViewState::Default => match self.tab {
                Tab::Detail => &[
                    (&["Esc", "Ctrl-c"], "Quit app"),
                    (&["h/l"], "Select tabs"),
                    (&["Backspace"], "Close detail panel"),
                    (&["j/k"], "Scroll forward/backward"),
                    (&["r"], "Open copy dialog"),
                    (&["s"], "Download object"),
                    (&["S"], "Download object as"),
                    (&["p"], "Preview object"),
                    (&["x"], "Open management console in browser"),
                ],
                Tab::Version => &[
                    (&["Esc", "Ctrl-c"], "Quit app"),
                    (&["h/l"], "Select tabs"),
                    (&["j/k"], "Select version"),
                    (&["g/G"], "Go to top/bottom"),
                    (&["Backspace"], "Close detail panel"),
                    (&["r"], "Open copy dialog"),
                    (&["s"], "Download object"),
                    (&["S"], "Download object as"),
                    (&["p"], "Preview object"),
                    (&["x"], "Open management console in browser"),
                ],
            },
            ViewState::SaveDialog(_) => &[
                (&["Ctrl-c"], "Quit app"),
                (&["Esc"], "Close save dialog"),
                (&["Enter"], "Download object"),
            ],
            ViewState::CopyDetailDialog(_) => &[
                (&["Ctrl-c"], "Quit app"),
                (&["Esc", "Backspace"], "Close copy dialog"),
                (&["j/k"], "Select item"),
                (&["Enter"], "Copy selected value to clipboard"),
            ],
        };
        build_helps(helps)
    }

    pub fn short_helps(&self) -> Vec<(String, usize)> {
        let helps: &[(&[&str], &str, usize)] = match self.view_state {
            ViewState::Default => match self.tab {
                Tab::Detail => &[
                    (&["Esc"], "Quit", 0),
                    (&["h/l"], "Select tabs", 3),
                    (&["j/k"], "Scroll", 5),
                    (&["s/S"], "Download", 1),
                    (&["p"], "Preview", 4),
                    (&["Backspace"], "Close", 2),
                    (&["?"], "Help", 0),
                ],
                Tab::Version => &[
                    (&["Esc"], "Quit", 0),
                    (&["h/l"], "Select tabs", 3),
                    (&["j/k"], "Select", 5),
                    (&["s/S"], "Download", 1),
                    (&["p"], "Preview", 4),
                    (&["Backspace"], "Close", 2),
                    (&["?"], "Help", 0),
                ],
            },
            ViewState::SaveDialog(_) => &[
                (&["Esc"], "Close", 2),
                (&["Enter"], "Download", 1),
                (&["?"], "Help", 0),
            ],
            ViewState::CopyDetailDialog(_) => &[
                (&["Esc"], "Close", 2),
                (&["j/k"], "Select", 3),
                (&["Enter"], "Copy", 1),
                (&["?"], "Help", 0),
            ],
        };

        build_short_helps(helps)
    }
}

impl ObjectDetailPage {
    fn toggle_tab(&mut self) {
        self.tab = self.tab.next();
    }

    fn open_save_dialog(&mut self) {
        self.view_state = ViewState::SaveDialog(InputDialogState::default());
    }

    pub fn close_save_dialog(&mut self) {
        self.view_state = ViewState::Default;
    }

    fn open_copy_detail_dialog(&mut self) {
        self.view_state = ViewState::CopyDetailDialog(CopyDetailDialogState::default());
    }

    fn close_copy_detail_dialog(&mut self) {
        self.view_state = ViewState::Default;
    }

    fn download(&self) {
        let file_detail = self.file_detail.clone();
        let version_id = self.current_selected_version_id();
        self.tx
            .send(AppEventType::DetailDownloadObject(file_detail, version_id));
    }

    fn download_as(&self, input: String) {
        let input: String = input.trim().into();
        if input.is_empty() {
            return;
        }

        let file_detail = self.file_detail.clone();
        let version_id = self.current_selected_version_id();
        self.tx.send(AppEventType::DetailDownloadObjectAs(
            file_detail,
            input,
            version_id,
        ));
    }

    fn preview(&self) {
        let file_detail = self.file_detail.clone();
        let version_id = self.current_selected_version_id();
        self.tx
            .send(AppEventType::OpenPreview(file_detail, version_id));
    }

    fn open_management_console(&self) {
        let file_name = self.file_detail.name.clone();
        self.tx
            .send(AppEventType::ObjectDetailOpenManagementConsole(file_name));
    }

    fn current_selected_version_id(&self) -> Option<String> {
        match self.tab {
            Tab::Detail => None,
            Tab::Version => self
                .file_versions
                .get(self.version_tab_state.selected)
                .map(|v| v.version_id.clone()),
        }
    }
}

fn build_list_items_from_object_items(
    current_items: &[ObjectItem],
    offset: usize,
    selected: usize,
    area: Rect,
) -> Vec<ListItem> {
    let show_item_count = (area.height as usize) - 2 /* border */;
    current_items
        .iter()
        .skip(offset)
        .take(show_item_count)
        .enumerate()
        .map(|(idx, item)| build_list_item_from_object_item(idx, item, offset, selected, area))
        .collect()
}

fn build_list_item_from_object_item(
    idx: usize,
    item: &ObjectItem,
    offset: usize,
    selected: usize,
    area: Rect,
) -> ListItem {
    let content = match item {
        ObjectItem::Dir { name, .. } => {
            let content = format_dir_item(name, area.width);
            let style = Style::default().add_modifier(Modifier::BOLD);
            Span::styled(content, style)
        }
        ObjectItem::File { name, .. } => {
            let content = format_file_item(name, area.width);
            let style = Style::default();
            Span::styled(content, style)
        }
    };
    if idx + offset == selected {
        ListItem::new(content).style(
            Style::default()
                .bg(SELECTED_DISABLED_COLOR)
                .fg(SELECTED_ITEM_TEXT_COLOR),
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

fn format_file_item(name: &str, width: u16) -> String {
    let name_w: usize = (width as usize) - 2 /* spaces */ - 4 /* border */;
    format!(" {:<name_w$} ", name, name_w = name_w)
}

fn build_tabs(tab: Tab) -> Tabs<'static> {
    let tabs = vec!["Detail", "Version"];
    Tabs::new(tabs)
        .select(tab.val())
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(SELECTED_COLOR),
        )
        .block(Block::default().borders(Borders::BOTTOM))
}

fn build_detail_content_lines(detail: &FileDetail) -> Vec<Line<'static>> {
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

    flatten_with_empty_lines(details)
}

#[derive(Debug)]
struct DetailTabState {
    scroll_lines_state: ScrollLinesState,
}

impl DetailTabState {
    fn new(file_detail: &FileDetail) -> Self {
        let scroll_lines = build_detail_content_lines(file_detail);
        let scroll_lines_state =
            ScrollLinesState::new(scroll_lines, ScrollLinesOptions::new(false, true));
        Self { scroll_lines_state }
    }
}

#[derive(Debug, Default)]
struct DetailTab {}

impl StatefulWidget for DetailTab {
    type State = DetailTabState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let scroll_lines = ScrollLines::default();
        StatefulWidget::render(scroll_lines, area, buf, &mut state.scroll_lines_state);
    }
}

fn build_help_lines(versions: &[FileVersion]) -> Vec<Vec<Line<'static>>> {
    versions
        .iter()
        .map(|v| {
            let version_id = format_version(&v.version_id).to_owned();
            let last_modified = format_datetime(&v.last_modified);
            let size_byte = format_size_byte(v.size_byte);
            vec![
                Line::from(vec![
                    "   Version ID: ".add_modifier(Modifier::BOLD),
                    Span::raw(version_id),
                ]),
                Line::from(vec![
                    "Last Modified: ".add_modifier(Modifier::BOLD),
                    Span::raw(last_modified),
                ]),
                Line::from(vec![
                    "         Size: ".add_modifier(Modifier::BOLD),
                    Span::raw(size_byte),
                ]),
            ]
        })
        .collect()
}

#[derive(Debug, Default)]
struct VersionTabState {
    help_lines: Vec<Vec<Line<'static>>>,
    selected: usize,
    offset: usize,
    height: usize,
}

impl VersionTabState {
    fn new(versions: &[FileVersion]) -> Self {
        let help_lines = build_help_lines(versions);
        Self {
            help_lines,
            ..Default::default()
        }
    }

    fn select_next(&mut self) {
        if self.selected >= self.help_lines.len() - 1 {
            return;
        }

        self.selected += 1;

        let mut total_height = 0;
        for lines in self
            .help_lines
            .iter()
            .skip(self.offset)
            .take(self.selected - self.offset + 1)
        {
            total_height += lines.len();
            total_height += 1; // divider
        }
        if total_height > self.height {
            self.offset += 1;
        }
    }

    fn select_prev(&mut self) {
        if self.selected == 0 {
            return;
        }

        self.selected -= 1;
        if self.selected < self.offset {
            self.offset -= 1;
        }
    }

    fn select_first(&mut self) {
        self.selected = 0;
        self.offset = 0;
    }

    fn select_last(&mut self) {
        self.selected = self.help_lines.len() - 1;

        let mut total_height = 0;
        for (i, lines) in self.help_lines.iter().enumerate().rev() {
            total_height += lines.len();
            total_height += 1; // divider

            // https://github.com/rust-lang/rust-clippy/issues/4725
            #[allow(clippy::comparison_chain)]
            if total_height == self.height {
                self.offset = i;
                break;
            } else if total_height > self.height {
                self.offset = i + 1;
                break;
            }
        }
    }
}

#[derive(Debug, Default)]
struct VersionTab {}

impl StatefulWidget for VersionTab {
    type State = VersionTabState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // update state
        state.height = area.height as usize;

        let mut area = area;

        for (i, lines) in state.help_lines.iter().enumerate().skip(state.offset) {
            let lines_count = lines.len() as u16;

            if area.height < lines_count {
                let version_paragraph = Paragraph::new("⋮").alignment(Alignment::Center);
                version_paragraph.render(area, buf);
                break;
            }

            let divider_area_height = if area.height > lines_count { 1 } else { 0 };

            let chunks = Layout::vertical([
                Constraint::Length(lines_count),
                Constraint::Length(divider_area_height),
                Constraint::Min(0),
            ])
            .split(area);
            area = chunks[2];

            let divider = Divider::default();
            divider.render(chunks[1], buf);

            let chunks =
                Layout::horizontal([Constraint::Length(1), Constraint::Min(0)]).split(chunks[0]);

            let version_paragraph = Paragraph::new(lines.clone()).block(
                Block::default()
                    .borders(Borders::NONE)
                    .padding(Padding::left(1)),
            );
            if i == state.selected {
                let bar = Bar::default().color(SELECTED_COLOR);
                bar.render(chunks[0], buf);
            }
            version_paragraph.render(chunks[1], buf);
        }
    }
}

fn flatten_with_empty_lines(line_groups: Vec<Vec<Line>>) -> Vec<Line> {
    let n = line_groups.len();
    let mut ret: Vec<Line> = Vec::new();
    for (i, lines) in line_groups.into_iter().enumerate() {
        for line in lines {
            ret.push(line);
        }
        if i != n - 1 {
            ret.push(Line::from(""));
        }
    }
    ret
}

#[cfg(test)]
mod tests {
    use crate::{event, set_cells};

    use super::*;
    use chrono::{DateTime, Local, NaiveDateTime};
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

    #[test]
    fn test_render_detail_tab() -> std::io::Result<()> {
        let (tx, _) = event::new();
        let mut terminal = setup_terminal()?;

        terminal.draw(|f| {
            let (items, file_detail, file_versions) = fixtures();
            let items_len = items.len();
            let mut page = ObjectDetailPage::new(
                file_detail,
                file_versions,
                items,
                ScrollListState::new(items_len),
                tx,
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
        set_cells! { expected =>
            // selected item
            (2..28, [1]) => bg: Color::DarkGray, fg: Color::Black,
            // "Detail" is selected
            (32..38, [1]) => fg: Color::Cyan, modifier: Modifier::BOLD,
            // "Name" label
            (32..37, [3]) => modifier: Modifier::BOLD,
            // "Size" label
            (32..37, [6]) => modifier: Modifier::BOLD,
            // "Last Modified" label
            (32..46, [9]) => modifier: Modifier::BOLD,
            // "ETag" label
            (32..37, [12]) => modifier: Modifier::BOLD,
            // "Content-Type" label
            (32..45, [16]) => modifier: Modifier::BOLD,
        }

        terminal.backend().assert_buffer(&expected);

        Ok(())
    }

    #[test]
    fn test_render_version_tab() -> std::io::Result<()> {
        let (tx, _) = event::new();
        let mut terminal = setup_terminal()?;

        terminal.draw(|f| {
            let (items, file_detail, file_versions) = fixtures();
            let items_len = items.len();
            let mut page = ObjectDetailPage::new(
                file_detail,
                file_versions,
                items,
                ScrollListState::new(items_len),
                tx,
            );
            page.toggle_tab();
            let area = Rect::new(0, 0, 60, 20);
            page.render(f, area);
        })?;

        #[rustfmt::skip]
        let mut expected = Buffer::with_lines([
            "┌───────────────────── 1 / 3 ┐┌────────────────────────────┐",
            "│  file1                     ││ Detail │ Version           │",
            "│  file2                     ││────────────────────────────│",
            "│  file3                     ││┃    Version ID: 60f36bc2-0f│",
            "│                            ││┃ Last Modified: 2024-01-02 │",
            "│                            ││┃          Size: 1.01 KiB   │",
            "│                            ││────────────────────────────│",
            "│                            ││     Version ID: 1c5d3bcc-2b│",
            "│                            ││  Last Modified: 2024-01-01 │",
            "│                            ││           Size: 1 KiB      │",
            "│                            ││────────────────────────────│",
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
        set_cells! { expected =>
            // selected item
            (2..28, [1]) => bg: Color::DarkGray, fg: Color::Black,
            // "Version" is selected
            (41..48, [1]) => fg: Color::Cyan, modifier: Modifier::BOLD,
            // "Version ID" label
            (33..48, [3, 7]) => modifier: Modifier::BOLD,
            // "Last Modified" label
            (33..48, [4, 8]) => modifier: Modifier::BOLD,
            // "Size" label
            (33..48, [5, 9]) => modifier: Modifier::BOLD,
            // selected bar
            ([31], [3, 4, 5]) => fg: Color::Cyan,
            // divider
            (31..59, [6, 10]) => fg: Color::DarkGray,
        }

        terminal.backend().assert_buffer(&expected);

        Ok(())
    }

    #[test]
    fn test_render_save_dialog_detail_tab() -> std::io::Result<()> {
        let (tx, _) = event::new();
        let mut terminal = setup_terminal()?;

        terminal.draw(|f| {
            let (items, file_detail, file_versions) = fixtures();
            let items_len = items.len();
            let mut page = ObjectDetailPage::new(
                file_detail,
                file_versions,
                items,
                ScrollListState::new(items_len),
                tx,
            );
            page.open_save_dialog();
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
            "│         ╭Save As───────────────────────────────╮         │",
            "│         │                                      │         │",
            "│         ╰──────────────────────────────────────╯ 2       │",
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
        set_cells! { expected =>
            // selected item
            (2..28, [1]) => bg: Color::DarkGray, fg: Color::Black,
            // "Detail" is selected
            (32..38, [1]) => fg: Color::Cyan, modifier: Modifier::BOLD,
            // "Name" label
            (32..37, [3]) => modifier: Modifier::BOLD,
            // "Size" label
            (32..37, [6]) => modifier: Modifier::BOLD,
            // "ETag" label
            (32..37, [12]) => modifier: Modifier::BOLD,
            // "Content-Type" label
            (32..45, [16]) => modifier: Modifier::BOLD,
        }

        terminal.backend().assert_buffer(&expected);

        Ok(())
    }

    #[test]
    fn test_render_copy_detail_dialog_detail_tab() -> std::io::Result<()> {
        let (tx, _) = event::new();
        let mut terminal = setup_terminal()?;

        terminal.draw(|f| {
            let (items, file_detail, file_versions) = fixtures();
            let items_len = items.len();
            let mut page = ObjectDetailPage::new(
                file_detail,
                file_versions,
                items,
                ScrollListState::new(items_len),
                tx,
            );
            page.open_copy_detail_dialog();
            let area = Rect::new(0, 0, 60, 20);
            page.render(f, area);
        })?;

        #[rustfmt::skip]
        let mut expected = Buffer::with_lines([
            "┌───────────────────── 1 / 3 ┐┌────────────────────────────┐",
            "│  file1                     ││ Detail │ Version           │",
            "│  file2                     ││────────────────────────────│",
            "│  file3                     ││ Name:                      │",
            "│ ╭Copy──────────────────────────────────────────────────╮ │",
            "│ │ Key:                                                 │ │",
            "│ │   file1                                              │ │",
            "│ │ S3 URI:                                              │ │",
            "│ │   s3://bucket-1/file1                                │ │",
            "│ │ ARN:                                                 │ │",
            "│ │   arn:aws:s3:::bucket-1/file1                        │ │",
            "│ │ Object URL:                                          │ │",
            "│ │   https://bucket-1.s3.ap-northeast-1.amazonaws.com/f │ │",
            "│ │ ETag:                                                │ │",
            "│ │   bef684de-a260-48a4-8178-8a535ecccadb               │ │",
            "│ ╰──────────────────────────────────────────────────────╯ │",
            "│                            ││ Content-Type:              │",
            "│                            ││  text/plain                │",
            "│                            ││                            │",
            "└────────────────────────────┘└────────────────────────────┘",
        ]);
        set_cells! { expected =>
            // selected item
            (2..28, [1]) => bg: Color::DarkGray, fg: Color::Black,
            // "Detail" is selected
            (32..38, [1]) => fg: Color::Cyan, modifier: Modifier::BOLD,
            // "Name" label
            (32..37, [3]) => modifier: Modifier::BOLD,
            // "Content-Type" label
            (32..45, [16]) => modifier: Modifier::BOLD,
            // "Key" label
            (4..8, [5]) => modifier: Modifier::BOLD,
            // "S3 URI" label
            (4..11, [7]) => modifier: Modifier::BOLD,
            // "ARN" label
            (4..8, [9]) => modifier: Modifier::BOLD,
            // "Object URL" label
            (4..15, [11]) => modifier: Modifier::BOLD,
            // "ETag" label
            (4..9, [13]) => modifier: Modifier::BOLD,
            // "Key" is selected
            (4..56, [5, 6]) => fg: Color::Cyan,
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
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
            .unwrap()
            .and_local_timezone(Local)
            .unwrap()
    }

    fn fixtures() -> (Vec<ObjectItem>, FileDetail, Vec<FileVersion>) {
        let items = vec![
            ObjectItem::File {
                name: "file1".to_string(),
                size_byte: 1024 + 10,
                last_modified: parse_datetime("2024-01-02 13:01:02"),
            },
            ObjectItem::File {
                name: "file2".to_string(),
                size_byte: 1024 * 999,
                last_modified: parse_datetime("2023-12-31 09:00:00"),
            },
            ObjectItem::File {
                name: "file3".to_string(),
                size_byte: 1024,
                last_modified: parse_datetime("2024-01-03 12:59:59"),
            },
        ];
        let file_detail = FileDetail {
            name: "file1".to_string(),
            size_byte: 1024 + 10,
            last_modified: parse_datetime("2024-01-02 13:01:02"),
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
                last_modified: parse_datetime("2024-01-02 13:01:02"),
                is_latest: true,
            },
            FileVersion {
                version_id: "1c5d3bcc-2bb3-4cd5-875f-a95a6ae53f65".to_string(),
                size_byte: 1024,
                last_modified: parse_datetime("2024-01-01 23:59:59"),
                is_latest: false,
            },
        ];
        (items, file_detail, file_versions)
    }
}
