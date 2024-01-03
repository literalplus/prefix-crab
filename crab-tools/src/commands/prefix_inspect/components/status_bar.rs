use tuirealm::command::{Cmd, CmdResult};
use tuirealm::event::{Key, KeyEvent};
use tuirealm::props::{Alignment, Color, Style, TextModifiers};
use tuirealm::tui::layout::Rect;
use tuirealm::tui::widgets::Paragraph;
use tuirealm::{
    AttrValue, Attribute, Component, Event, Frame, MockComponent, NoUserEvent, Props, State,
};

use super::super::Msg;

pub const PLACEHOLDER_ATTR: Attribute = Attribute::Custom("Placeholder");

pub struct StatusBar {
    props: Props,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self {
            props: Props::default(),
        }.text("Loading...")
    }
}

impl StatusBar {
    pub fn text<S>(mut self, s: S) -> Self
    where
        S: AsRef<str>,
    {
        self.attr(Attribute::Text, AttrValue::String(s.as_ref().to_string()));
        self
    }
}

impl MockComponent for StatusBar {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // Check if visible
        if self.props.get_or(Attribute::Display, AttrValue::Flag(true)) != AttrValue::Flag(true) {
            return;
        }
        // Get properties
        let mut text = self
            .props
            .get_or(Attribute::Text, AttrValue::String(String::default()))
            .unwrap_string();
        if text.is_empty() {
            text = self
                .props
                .get_or(PLACEHOLDER_ATTR, AttrValue::String("q Quit".to_string()))
                .unwrap_string();
        }
        let alignment = self
            .props
            .get_or(Attribute::TextAlign, AttrValue::Alignment(Alignment::Left))
            .unwrap_alignment();
        let foreground = self
            .props
            .get_or(Attribute::Foreground, AttrValue::Color(Color::White))
            .unwrap_color();
        let background = self
            .props
            .get_or(Attribute::Background, AttrValue::Color(Color::Black))
            .unwrap_color();
        let modifiers = self
            .props
            .get_or(
                Attribute::TextProps,
                AttrValue::TextModifiers(TextModifiers::BOLD),
            )
            .unwrap_text_modifiers();
        frame.render_widget(
            Paragraph::new(format!("  {}", text))
                .style(
                    Style::default()
                        .fg(foreground)
                        .bg(background)
                        .add_modifier(modifiers),
                )
                .alignment(alignment),
            area,
        );
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.props.get(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == PLACEHOLDER_ATTR {
            let value = value.unwrap_string();
            let suffixed = if value.is_empty() {
                "q Quit".to_string()
            } else {
                format!("{} | q Quit", value)
            };
            self.props.set(attr, AttrValue::String(suffixed));
        } else {
            self.props.set(attr, value);
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _: Cmd) -> CmdResult {
        CmdResult::None
    }
}

impl Component<Msg, NoUserEvent> for StatusBar {
    fn on(&mut self, ev: Event<NoUserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Msg::AppClose),
            Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => Some(Msg::AppClose),
            _ => None,
        }
    }
}
