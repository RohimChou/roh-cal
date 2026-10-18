//! 一行編輯框：畫在最下面（操作說明的上面），整個 terminal 那麼寬

use crate::app::{App, Editor};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::Style,
    text::Line,
    widgets::{Block, BorderType, Clear, Paragraph},
};

/// 編輯框的高度：上框線、打字那一列、檢查結果那一列、下框線
const HEIGHT: u16 = 4;

/// 編輯框的位置：最下面操作說明的上面、整個 terminal 那麼寬；畫畫面和滑鼠（點框外 = 取消）都用這個
pub fn area(screen: Rect) -> Rect {
    let height = HEIGHT.min(screen.height.saturating_sub(1));
    Rect::new(
        screen.x,
        screen.bottom().saturating_sub(1 + height),
        screen.width,
        height,
    )
}

/// 畫編輯框：上框線寫在編第幾行、第 1 列是打的字（游標在打字的位置）、第 2 列即時顯示這行會讀成什麼
/// （有問題就是紅字，說明哪裡不對、存不了）、下框線寫怎麼存
pub fn render(frame: &mut Frame, editor: &Editor, app: &App) {
    let theme = &app.theme;
    let texts = app.lang.texts();
    let area = area(frame.area());
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme.focus))
        .title_top(Line::styled(
            texts.edit_title(editor.item.line),
            Style::new().fg(theme.label),
        ))
        .title_bottom(Line::styled(
            texts.editor_keys(),
            Style::new().fg(theme.muted),
        ));
    let inner = block.inner(area);
    // 先清掉底下的月曆，再畫框
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    // 打的字：太長時往左捲，游標一直看得到（前面留一格空白）
    let input_area = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(1),
        height: 1,
        ..inner
    };
    let width = usize::from(input_area.width);
    let scroll = editor.input.visual_scroll(width.saturating_sub(1));
    frame.render_widget(
        Paragraph::new(editor.input.value())
            .style(Style::new().fg(theme.text))
            .scroll((0, u16::try_from(scroll).unwrap_or(u16::MAX))),
        input_area,
    );
    let cursor = editor.input.visual_cursor().saturating_sub(scroll);
    frame.set_cursor_position(Position::new(
        input_area.x + u16::try_from(cursor).unwrap_or(0),
        input_area.y,
    ));
    // 檢查結果：讀成什麼（淡色），或哪裡不對（紅字）
    if inner.height > 1 {
        let check = match editor.check() {
            Ok(item) => Line::styled(texts.preview(&item), Style::new().fg(theme.muted)),
            Err(problem) => Line::styled(texts.problem(&problem), Style::new().fg(theme.error)),
        };
        frame.render_widget(
            check,
            Rect {
                y: inner.y + 1,
                height: 1,
                ..inner
            },
        );
    }
}
