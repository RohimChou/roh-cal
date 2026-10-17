//! 畫整個畫面：最上面一列標題、中間的框、最下面一列操作說明

use crate::app::App;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    widgets::{Block, BorderType},
};

/// 畫整個畫面
pub fn render(frame: &mut Frame, app: &App) {
    let texts = app.lang.texts();
    let [title, main, hints] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    frame.render_widget(texts.title, title);
    frame.render_widget(Block::bordered().border_type(BorderType::Rounded), main);
    frame.render_widget(texts.hints, hints);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texts::Lang;
    use ratatui::{Terminal, backend::TestBackend};

    /// 用 `lang` 畫一次 `width` × `height` 的畫面
    fn draw(lang: Lang, width: u16, height: u16) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| render(frame, &App::new(lang)))
            .unwrap();
        terminal
    }

    #[test]
    fn title_frame_and_hints_in_both_languages() {
        draw(Lang::En, 12, 5).backend().assert_buffer_lines([
            " roh-cal    ",
            "╭──────────╮",
            "│          │",
            "╰──────────╯",
            " q quit     ",
        ]);
        draw(Lang::ZhTw, 12, 5).backend().assert_buffer_lines([
            " roh-cal    ",
            "╭──────────╮",
            "│          │",
            "╰──────────╯",
            " q 離開     ",
        ]);
    }
}
