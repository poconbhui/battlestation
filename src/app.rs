use crate::runner::{self, Runner};

use iced::widget::{self, Column, Row};

pub struct App {
    runners: Vec<Runner>,
    runner_stdout_buf: Vec<String>,
    runner_stderr_buf: Vec<String>,
    logs: Vec<(usize, IO)>,
}

pub enum IO {
    Stdout(String),
    Stderr(String),
}

#[derive(Debug)]
pub enum Message {
    Runner(usize, runner::Message),
}

const GLYPH_STDOUT: &str = "[>]";
const GLYPH_STDERR: &str = "[!]";

impl App {
    pub fn new(runners: Vec<Runner>) -> App {
        let runner_stdout_buf = vec![String::new(); runners.len()];
        let runner_stderr_buf = vec![String::new(); runners.len()];
        App {
            runners,
            runner_stdout_buf,
            runner_stderr_buf,
            logs: Vec::new(),
        }
    }

    pub fn view(&self) -> iced::Element<Message> {
        let runners = Column::from_iter(
            self.runners
                .iter()
                .map(Runner::view)
                .enumerate()
                .map(|(i, el)| el.map(move |msg| Message::Runner(i, msg))),
        )
        .spacing(10);

        fn to_row<'a>(name: &'a str, glyph: &'a str, line: &'a str) -> iced::Element<'a, Message> {
            widget::row![
                widget::text(name).font(iced::Font::MONOSPACE),
                widget::text(glyph).font(iced::Font::MONOSPACE),
                widget::text(" ").font(iced::Font::MONOSPACE),
                widget::text(line).font(iced::Font::MONOSPACE),
            ].into()
        }
        fn to_row_io<'a>(name: &'a str, io: &'a IO) -> iced::Element<'a, Message> {
            let (glyph, line) = match io {
                IO::Stdout(line) => (GLYPH_STDOUT, line),
                IO::Stderr(line) => (GLYPH_STDERR, line),
            };
            to_row(name, glyph, line)
        }

        let logs = self.logs.iter()
            .rev()
            .filter(|(i,_)| self.runners[*i].show_logs)
            .take(1000)
            .map(|(i, io)| to_row_io(&self.runners[*i].name, io))
            .collect::<Vec<_>>()
            .into_iter()
            .rev();
        let mut logs = Column::from_iter(logs);
        for i in 0..self.runners.len() {
            if self.runner_stdout_buf[i].len() > 0 && self.runners[i].show_logs {
                let stdout = &self.runner_stdout_buf[i];
                logs = logs.push(to_row(&self.runners[i].name, GLYPH_STDOUT, stdout));
            }
            if self.runner_stderr_buf[i].len() > 0 && self.runners[i].show_logs {
                let stderr = &self.runner_stderr_buf[i];
                logs = logs.push(to_row(&self.runners[i].name, GLYPH_STDERR, stderr));
            }
        }
        let logs = widget::container(
                widget::scrollable(logs)
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill)
            )
            .style(|theme| {
                let mut style = widget::container::rounded_box(theme);
                style.background = Some(iced::Background::Color(theme.palette().background));
                style.border.color = theme.palette().text;
                style.border.width = 1.0;
                style.border.radius = 5.0.into();
                style
            })
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .padding(5);

        Row::from_iter([runners.into(), logs.into()])
            .padding(10)
            .spacing(10)
            .into()
    }

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Runner(i, message) => {
                if let runner::Message::Stdout(ref s) = message {
                    let mut s: &str = s;
                    // read until '\n'
                    while s.len() > 0 {
                        match s.find('\n') {
                            Some(n) => {
                                self.runner_stdout_buf[i].push_str(&s[..n]);
                                let line = std::mem::take(&mut self.runner_stdout_buf[i]);
                                self.logs.push((i, IO::Stdout(line)));
                                s = &s[n+1..];
                            }
                            None => {
                                self.runner_stdout_buf[i].push_str(&s[..]);
                                break;
                            }
                        };
                    }
                }
                if let runner::Message::Stderr(ref s) = message {
                    let mut s: &str = s;
                    // read until '\n'
                    while s.len() > 0 {
                        match s.find('\n') {
                            Some(n) => {
                                self.runner_stderr_buf[i].push_str(&s[..n]);
                                let line = std::mem::take(&mut self.runner_stderr_buf[i]);
                                self.logs.push((i, IO::Stderr(line)));
                                s = &s[n+1..];
                            }
                            None => {
                                self.runner_stderr_buf[i].push_str(&s[..]);
                                break;
                            }
                        };
                    }
                }

                let task = self.runners[i].update(message);
                task.map(move |msg| Message::Runner(i, msg))
            }
        }
    }
}
