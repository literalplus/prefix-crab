pub mod status_bar {
    use tuirealm::command::{Cmd, CmdResult};
    use tuirealm::props::{Alignment, Color, Style, TextModifiers};
    use tuirealm::tui::layout::Rect;
    use tuirealm::tui::widgets::Paragraph;
    use tuirealm::{
        AttrValue, Attribute, Component, Event, Frame, MockComponent, NoUserEvent, Props, State,
    };

    use super::super::Msg;

    pub struct StatusBar {
        props: Props,
    }

    impl Default for StatusBar {
        fn default() -> Self {
            Self {
                props: Props::default(),
            }
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

        pub fn alignment(mut self, a: Alignment) -> Self {
            self.attr(Attribute::TextAlign, AttrValue::Alignment(a));
            self
        }

        pub fn foreground(mut self, c: Color) -> Self {
            self.attr(Attribute::Foreground, AttrValue::Color(c));
            self
        }

        pub fn background(mut self, c: Color) -> Self {
            self.attr(Attribute::Background, AttrValue::Color(c));
            self
        }

        pub fn modifiers(mut self, m: TextModifiers) -> Self {
            self.attr(Attribute::TextProps, AttrValue::TextModifiers(m));
            self
        }
    }

    impl MockComponent for StatusBar {
        fn view(&mut self, frame: &mut Frame, area: Rect) {
            // Check if visible
            if self.props.get_or(Attribute::Display, AttrValue::Flag(true)) == AttrValue::Flag(true)
            {
                // Get properties
                let text = self
                    .props
                    .get_or(Attribute::Text, AttrValue::String(String::default()))
                    .unwrap_string();
                let alignment = self
                    .props
                    .get_or(Attribute::TextAlign, AttrValue::Alignment(Alignment::Left))
                    .unwrap_alignment();
                let foreground = self
                    .props
                    .get_or(Attribute::Foreground, AttrValue::Color(Color::Reset))
                    .unwrap_color();
                let background = self
                    .props
                    .get_or(Attribute::Background, AttrValue::Color(Color::Reset))
                    .unwrap_color();
                let modifiers = self
                    .props
                    .get_or(
                        Attribute::TextProps,
                        AttrValue::TextModifiers(TextModifiers::empty()),
                    )
                    .unwrap_text_modifiers();
                frame.render_widget(
                    Paragraph::new(text)
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
        }

        fn query(&self, attr: Attribute) -> Option<AttrValue> {
            self.props.get(attr)
        }

        fn attr(&mut self, attr: Attribute, value: AttrValue) {
            self.props.set(attr, value);
        }

        fn state(&self) -> State {
            State::None
        }

        fn perform(&mut self, _: Cmd) -> CmdResult {
            CmdResult::None
        }
    }

    impl Component<Msg, NoUserEvent> for StatusBar {
        fn on(&mut self, _: Event<NoUserEvent>) -> Option<Msg> {
            // Does nothing
            None
        }
    }
}

pub mod lhr_list {
    use tui_realm_stdlib::List;
    use tuirealm::{
        command::{Cmd, CmdResult, Direction, Position},
        event::{Key, KeyEvent},
        props::{Alignment, BorderType, Borders, Color, TableBuilder, TextSpan},
        Component, Event, MockComponent, NoUserEvent,
    };

    use crate::commands::prefix_inspect::Msg;

    #[derive(MockComponent)]
    pub struct LhrList {
        component: List,
    }

    impl LhrList {
        pub fn new(items: &[String]) -> Self {
            let mut table = TableBuilder::default();
            let mut i = 0;
            for item in items {
                table.add_col(TextSpan::from(format!("{}", i)).fg(Color::Cyan).italic());
                table.add_col(TextSpan::from(item));
                table.add_row();
            }
            Self {
                component: List::default()
                    .borders(
                        Borders::default()
                            .modifiers(BorderType::Rounded)
                            .color(Color::Yellow),
                    )
                    .title("Last-Hop Routers", Alignment::Center)
                    .scroll(true)
                    .highlighted_color(Color::LightYellow)
                    .highlighted_str("🚀")
                    .rewind(true)
                    .rows(table.build())
                    .selected_line(0),
            }
        }
    }

    impl Component<Msg, NoUserEvent> for LhrList {
        fn on(&mut self, ev: Event<NoUserEvent>) -> Option<Msg> {
            let _ = match ev {
                Event::Keyboard(KeyEvent {
                    code: Key::Down, ..
                }) => self.perform(Cmd::Move(Direction::Down)),
                Event::Keyboard(KeyEvent { code: Key::Up, .. }) => {
                    self.perform(Cmd::Move(Direction::Up))
                }
                Event::Keyboard(KeyEvent {
                    code: Key::PageDown,
                    ..
                }) => self.perform(Cmd::Scroll(Direction::Down)),
                Event::Keyboard(KeyEvent {
                    code: Key::PageUp, ..
                }) => self.perform(Cmd::Scroll(Direction::Up)),
                Event::Keyboard(KeyEvent {
                    code: Key::Home, ..
                }) => self.perform(Cmd::GoTo(Position::Begin)),
                Event::Keyboard(KeyEvent { code: Key::End, .. }) => {
                    self.perform(Cmd::GoTo(Position::End))
                }
                Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => return Some(Msg::AppClose),
                _ => CmdResult::None,
            };
            None
        }
    }
}

pub mod base_info {
    use ipnet::Ipv6Net;
    use tui_realm_stdlib::Table;
    use tuirealm::{
        props::{Alignment, BorderType, Borders, Color, TableBuilder, TextSpan},
        MockComponent, Component, NoUserEvent, Event, event::{KeyEvent, Key}, command::{Cmd, Direction, CmdResult},
    };

    use crate::commands::prefix_inspect::Msg;

    #[derive(MockComponent)]
    pub struct BaseInfo {
        component: Table,
    }

    impl BaseInfo {
        pub fn loading_placeholder(prefix: Ipv6Net) -> Self {
            let table = TableBuilder::default()
                .add_col(TextSpan::from(format!("{}", prefix)).fg(Color::Yellow))
                .build();
            Self {
                component: Table::default()
                    .borders(
                        Borders::default()
                            .modifiers(BorderType::Thick)
                            .color(Color::Yellow),
                    )
                    .title("Prefix Inspection", Alignment::Center)
                    .scroll(true)
                    .highlighted_color(Color::LightYellow)
                    .highlighted_str("🚀")
                    .rewind(true)
                    .step(4)
                    .row_height(1)
                    .headers(&["Key", "Msg", "Description"])
                    .column_spacing(3)
                    .widths(&[30, 20, 50])
                    .table(table),
            }
        }
    }

    impl Component<Msg, NoUserEvent> for BaseInfo {
        fn on(&mut self, ev: Event<NoUserEvent>) -> Option<Msg> {
            let _ = match ev {
                Event::Keyboard(KeyEvent {
                    code: Key::Down, ..
                }) => self.perform(Cmd::Move(Direction::Down)),
                Event::Keyboard(KeyEvent { code: Key::Up, .. }) => {
                    self.perform(Cmd::Move(Direction::Up))
                }
                Event::Keyboard(KeyEvent {
                    code: Key::PageDown,
                    ..
                }) => self.perform(Cmd::Scroll(Direction::Down)),
                Event::Keyboard(KeyEvent {
                    code: Key::PageUp, ..
                }) => self.perform(Cmd::Scroll(Direction::Up)),
                Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => return Some(Msg::AppClose),
                _ => return None,
            };
            None
        }
    }
}
