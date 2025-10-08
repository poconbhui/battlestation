use crate::runner::{self, Runner};

use iced::widget::{self, Column, Row};
use std::time::SystemTime;

pub struct App {
    runners: Vec<Runner>,
    runner_stdout_buf: Vec<String>,
    runner_stderr_buf: Vec<String>,
    logs: Vec<Vec<(SystemTime, IO)>>, // log[runner_id][log_item]

    scroll_state: scroll_state::ScrollState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum IO {
    Stdout(String),
    Stderr(String),
}

#[derive(Debug)]
pub enum Message {
    Runner(usize, runner::Message),
    ScrollState(scroll_state::Message),
}

const GLYPH_STDOUT: &str = "[>]";
const GLYPH_STDERR: &str = "[!]";

impl App {
    pub fn new(runners: Vec<Runner>) -> App {
        let runner_stdout_buf = vec![String::new(); runners.len()];
        let runner_stderr_buf = vec![String::new(); runners.len()];
        let logs = vec![Vec::new(); runners.len()];
        App {
            runners,
            runner_stdout_buf,
            runner_stderr_buf,
            logs,
            scroll_state: scroll_state::ScrollState::new(),
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
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
                iced::Element::from(widget::text(name).font(iced::Font::MONOSPACE)),
                iced::Element::from(widget::text(glyph).font(iced::Font::MONOSPACE)),
                iced::Element::from(widget::text(" ").font(iced::Font::MONOSPACE)),
                iced::Element::from(widget::text(line).font(iced::Font::MONOSPACE)),
            ]
            .into()
        }
        fn to_row_io<'a>(name: &'a str, io: &'a IO) -> iced::Element<'a, Message> {
            let (glyph, line) = match io {
                IO::Stdout(line) => (GLYPH_STDOUT, line),
                IO::Stderr(line) => (GLYPH_STDERR, line),
            };
            to_row(name, glyph, line)
        }

        let mut scroll_contents = Vec::<iced::Element<_>>::new();
        // culled lines before
        scroll_contents.push(
            widget::Space::with_height(iced::Length::Fixed(self.scroll_state.space_before)).into(),
        );
        // visible text
        scroll_contents.extend(self.scroll_state.logs.iter().map(|ssl| {
            to_row_io(
                &self.runners[ssl.runner_idx].name,
                &self.logs[ssl.runner_idx][ssl.log_pos].1,
            )
        }));
        // culled lines after
        scroll_contents.push(
            widget::Space::with_height(iced::Length::Fixed(self.scroll_state.space_after)).into(),
        );
        // most recent lines
        for i in 0..self.runners.len() {
            if !self.runner_stdout_buf[i].is_empty() && self.runners[i].show_logs {
                let stdout = &self.runner_stdout_buf[i];
                scroll_contents.push(to_row(&self.runners[i].name, GLYPH_STDOUT, stdout));
            }
            if !self.runner_stderr_buf[i].is_empty() && self.runners[i].show_logs {
                let stderr = &self.runner_stderr_buf[i];
                scroll_contents.push(to_row(&self.runners[i].name, GLYPH_STDERR, stderr));
            }
        }

        let logs = widget::container(
            widget::scrollable(Column::from_vec(scroll_contents))
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .on_scroll(|v| Message::ScrollState(scroll_state::Message::OnScroll(v)))
                .id(self.scroll_state.id.clone())
                .anchor_y(self.scroll_state.anchor_y),
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
                let task = self.runners[i].update(message.clone());
                let mut task = task.map(move |msg| Message::Runner(i, msg));

                match message {
                    runner::Message::Stdout(ref s) => {
                        let mut s: &str = s;
                        // read until '\n'
                        while !s.is_empty() {
                            match s.find('\n') {
                                Some(n) => {
                                    self.runner_stdout_buf[i].push_str(&s[..n]);
                                    let line = std::mem::take(&mut self.runner_stdout_buf[i]);
                                    self.logs[i].push((SystemTime::now(), IO::Stdout(line)));
                                    s = &s[n + 1..];
                                }
                                None => {
                                    self.runner_stdout_buf[i].push_str(s);
                                    break;
                                }
                            };
                        }

                        if self.runners[i].show_logs {
                            let scroll_task = self
                                .scroll_state
                                .update_logs(&self.logs)
                                .map(Message::ScrollState);
                            task = iced::Task::batch([task, scroll_task]);
                        }
                    }

                    runner::Message::Stderr(ref s) => {
                        let mut s: &str = s;
                        // read until '\n'
                        while !s.is_empty() {
                            match s.find('\n') {
                                Some(n) => {
                                    self.runner_stderr_buf[i].push_str(&s[..n]);
                                    let line = std::mem::take(&mut self.runner_stderr_buf[i]);
                                    self.logs[i].push((SystemTime::now(), IO::Stderr(line)));
                                    s = &s[n + 1..];
                                }
                                None => {
                                    self.runner_stderr_buf[i].push_str(s);
                                    break;
                                }
                            };
                        }

                        if self.runners[i].show_logs {
                            let scroll_task = self
                                .scroll_state
                                .update_logs(&self.logs)
                                .map(Message::ScrollState);
                            task = iced::Task::batch([task, scroll_task]);
                        }
                    }

                    runner::Message::SetShowLogs(_) => {
                        let scroll_task = self
                            .scroll_state
                            .set_runner_idxs(
                                self.runners
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, r)| r.show_logs)
                                    .map(|(i, _)| i),
                            )
                            .map(Message::ScrollState);

                        task = iced::Task::batch([task, scroll_task]);
                    }
                    _ => (),
                }

                task
            }

            Message::ScrollState(message) => self
                .scroll_state
                .update(message, &self.logs)
                .map(Message::ScrollState),
        }
    }
}

mod scroll_state {
    use crate::app::IO;

    use iced::widget;
    use std::time::SystemTime;

    pub struct ScrollState {
        pub id: widget::scrollable::Id,
        pub space_after: f32,
        pub space_before: f32,
        pub anchor_y: widget::scrollable::Anchor,
        pub logs: Vec<ScrollStateLog>,
        pub viewport: Option<Viewport>,
        runner_idxs: Vec<usize>,
        cursors: Vec<usize>,
        enable_updates: bool,
    }

    #[derive(Debug)]
    pub struct Viewport {
        pub offset_top: widget::scrollable::AbsoluteOffset,
        pub offset_bottom: widget::scrollable::AbsoluteOffset,
        pub bounds: iced::Rectangle,
    }

    pub struct ScrollStateLog {
        pub runner_idx: usize,
        pub log_pos: usize,
    }

    #[derive(Debug)]
    pub enum Message {
        OnScroll(widget::scrollable::Viewport),
        UpdateLogs,
        SetEnableUpdates(bool),
    }

    impl ScrollState {
        pub fn new() -> ScrollState {
            ScrollState {
                id: widget::scrollable::Id::unique(),
                space_before: 0.0,
                space_after: 0.0,
                runner_idxs: Vec::new(),
                logs: Vec::new(),
                viewport: None,
                cursors: Vec::new(),
                anchor_y: widget::scrollable::Anchor::End,
                enable_updates: true,
            }
        }

        fn line_height() -> f32 {
            let iced::Pixels(line_height) = widget::text::LineHeight::default()
                .to_absolute(iced::Settings::default().default_text_size);
            line_height
        }

        pub fn update(
            &mut self,
            message: Message,
            runner_logs: &[Vec<(SystemTime, IO)>],
        ) -> iced::Task<Message> {
            match message {
                Message::UpdateLogs => self.update_logs(runner_logs),

                Message::OnScroll(viewport) => {
                    if !self.enable_updates {
                        return iced::Task::none();
                    }

                    // set internal viewport
                    match self.anchor_y {
                        widget::scrollable::Anchor::Start => {
                            self.viewport = Some(Viewport {
                                offset_top: viewport.absolute_offset(),
                                offset_bottom: viewport.absolute_offset_reversed(),
                                bounds: viewport.bounds(),
                            });
                        }
                        widget::scrollable::Anchor::End => {
                            self.viewport = Some(Viewport {
                                offset_top: viewport.absolute_offset_reversed(),
                                offset_bottom: viewport.absolute_offset(),
                                bounds: viewport.bounds(),
                            });
                        }
                    }

                    let update_task = self.update_logs(runner_logs);

                    // allow anchor release
                    let line_height = Self::line_height();
                    let scroll_task = match self.anchor_y {
                        widget::scrollable::Anchor::Start => {
                            if viewport.absolute_offset_reversed().y < 2.1 * line_height {
                                self.anchor_y = widget::scrollable::Anchor::End;
                                for i in 0..self.cursors.len() {
                                    let len = runner_logs[self.runner_idxs[i]].len();
                                    self.cursors[i] = len - self.cursors[i];
                                }

                                self.enable_updates = false;
                                widget::scrollable::scroll_to(
                                    self.id.clone(),
                                    widget::scrollable::AbsoluteOffset { x: 0.0, y: 0.0 },
                                )
                                .chain(iced::Task::done(Message::SetEnableUpdates(true)))
                                .chain(iced::Task::done(Message::UpdateLogs))
                            } else {
                                iced::Task::none()
                            }
                        }
                        widget::scrollable::Anchor::End => {
                            if viewport.absolute_offset().y > 2.1 * line_height {
                                self.anchor_y = widget::scrollable::Anchor::Start;
                                for i in 0..self.cursors.len() {
                                    let len = runner_logs[self.runner_idxs[i]].len();
                                    self.cursors[i] = len - self.cursors[i];
                                }

                                self.enable_updates = false;
                                widget::scrollable::scroll_to(
                                    self.id.clone(),
                                    viewport.absolute_offset_reversed(),
                                )
                                .chain(iced::Task::done(Message::SetEnableUpdates(true)))
                                .chain(iced::Task::done(Message::UpdateLogs))
                            } else {
                                iced::Task::none()
                            }
                        }
                    };

                    iced::Task::batch([update_task, scroll_task])
                }

                Message::SetEnableUpdates(v) => {
                    self.enable_updates = v;
                    iced::Task::none()
                }
            }
        }

        pub fn set_runner_idxs(
            &mut self,
            runner_idxs: impl Iterator<Item = usize>,
        ) -> iced::Task<Message> {
            self.runner_idxs.clear();
            self.runner_idxs.extend(runner_idxs);
            self.anchor_y = widget::scrollable::Anchor::End;
            self.cursors = vec![0; self.runner_idxs.len()];
            self.viewport = None;

            self.enable_updates = false;
            widget::scrollable::scroll_to(
                self.id.clone(),
                widget::scrollable::AbsoluteOffset { x: 0.0, y: 0.0 },
            )
            .chain(iced::Task::done(Message::SetEnableUpdates(true)))
            .chain(iced::Task::done(Message::UpdateLogs))
        }

        pub fn update_logs(
            &mut self,
            runner_logs: &[Vec<(SystemTime, IO)>],
        ) -> iced::Task<Message> {
            debug_assert!(
                self.runner_idxs.is_empty()
                    || self.runner_idxs.iter().max().unwrap_or(&0) < &runner_logs.len()
            );

            if !self.enable_updates {
                return iced::Task::none();
            }

            self.logs.clear();

            let line_height = Self::line_height();

            let mut total_lines = 0;
            for i in 0..self.runner_idxs.len() {
                total_lines += runner_logs[self.runner_idxs[i]].len();
            }

            // Number of lines visible in the viewport (rounded up)
            let mut n_visible_lines: usize = total_lines;
            if let Some(viewport) = &self.viewport {
                let visible_size = viewport.bounds.height;
                n_visible_lines = unsafe {
                    (visible_size / line_height)
                        .ceil()
                        .to_int_unchecked::<usize>()
                };
            }

            let mut n_lines_before;
            let mut n_lines_after;

            match self.anchor_y {
                widget::scrollable::Anchor::End => {
                    // Anchored to end, prefer stable n_lines_after

                    // Number of lines cut off by the bottom of the viewport
                    // (accuracy for big logs most important at the bottom)
                    n_lines_after = 0;
                    if let Some(viewport) = &self.viewport {
                        let offset_bottom = viewport.offset_bottom.y;
                        n_lines_after =
                            unsafe { (offset_bottom / line_height).floor().to_int_unchecked() };
                    }

                    // Ensure numbers match with total_lines
                    // (total lines or viewport might have changed incompatibly
                    //  since last update)
                    if n_visible_lines + n_lines_after > total_lines {
                        let mut diff = n_lines_after + n_visible_lines - total_lines;
                        if n_lines_after >= diff {
                            // take it all off n_lines_after
                            n_lines_after -= diff;
                        } else {
                            // take what we can off n_lines_after,
                            // take the rest off n_visible_lines
                            diff -= n_lines_after;
                            n_lines_after = 0;
                            assert!(n_visible_lines >= diff);
                            n_visible_lines -= diff;
                        }
                    }
                    assert!(n_visible_lines + n_lines_after <= total_lines);

                    // We want about 10 lines above and below the rendered viewport
                    if n_lines_after >= 10 {
                        n_lines_after -= 10;
                        n_visible_lines += 10;
                    } else {
                        n_visible_lines += n_lines_after;
                        n_lines_after = 0;
                    }

                    n_lines_before = total_lines - n_lines_after - n_visible_lines;
                    if n_lines_before >= 10 {
                        n_visible_lines += 10;
                        n_lines_before -= 10;
                    } else {
                        n_visible_lines += n_lines_before;
                        n_lines_before = 0;
                    }
                }
                widget::scrollable::Anchor::Start => {
                    // Anchored to start, prefer stable n_lines_before

                    // Number of lines cut off by top of viewport
                    n_lines_before = 0;
                    if let Some(viewport) = &self.viewport {
                        let offset_top = viewport.offset_top.y;
                        n_lines_before =
                            unsafe { (offset_top / line_height).floor().to_int_unchecked() };
                    }

                    // Ensure numbers match with total_lines
                    // (total lines or viewport might have changed incompatibly
                    //  since last update)
                    if n_visible_lines + n_lines_before > total_lines {
                        let mut diff = n_lines_before + n_visible_lines - total_lines;
                        if n_lines_before >= diff {
                            // take it all off n_lines_before
                            n_lines_before -= diff;
                        } else {
                            // take what we can off n_lines_before,
                            // take the rest off n_visible_lines
                            diff -= n_lines_before;
                            n_lines_before = 0;
                            assert!(n_visible_lines >= diff);
                            n_visible_lines -= diff;
                        }
                    }
                    assert!(n_visible_lines + n_lines_before <= total_lines);

                    // We want about 10 lines above and below the rendered viewport
                    if n_lines_before >= 10 {
                        n_lines_before -= 10;
                        n_visible_lines += 10;
                    } else {
                        n_visible_lines += n_lines_before;
                        n_lines_before = 0;
                    }

                    n_lines_after = total_lines - n_lines_before - n_visible_lines;
                    if n_lines_after >= 10 {
                        n_visible_lines += 10;
                        n_lines_after -= 10;
                    } else {
                        n_visible_lines += n_lines_after;
                        n_lines_after = 0;
                    }
                }
            }

            //self.height       = (total_lines as f32) * line_height;
            self.space_before = (n_lines_before as f32) * line_height;
            self.space_after = (n_lines_after as f32) * line_height;

            let lens = self
                .runner_idxs
                .iter()
                .map(|i| runner_logs[*i].len())
                .collect::<Vec<_>>(); // start at end
            let mut cursors = self.cursors.clone();

            // If Anchor is START, stored cursors are from log start
            // If Anchor is END,   stored cursors are from log end

            let mut cursor_total = cursors.iter().sum::<usize>();

            // Logs ordered by (DATE DESC, LOGGER ASC)
            // e.g. 2025-01-01 3
            //      2025-01-01 2
            //      2025-01-01 1

            match self.anchor_y {
                widget::scrollable::Anchor::End => {
                    // Zipper merge of logs, ordered by log time

                    // Rewind cursors if they're ahead
                    // (travelling down the stack)
                    while cursor_total > n_lines_after {
                        let mut next: Option<(_, SystemTime)> = None;
                        for i in (0..self.runner_idxs.len()).rev() {
                            if cursors[i] == 0 {
                                continue;
                            } // cursor at start
                            let pos = lens[i] - cursors[i];
                            let log = &runner_logs[self.runner_idxs[i]][pos];

                            match next {
                                None => {
                                    next = Some((i, log.0));
                                }
                                Some((_, t)) => {
                                    if log.0 <= t {
                                        // if times match, prefer lower log idx
                                        next = Some((i, log.0));
                                    }
                                }
                            }
                        }

                        match next {
                            Some((i, _)) => {
                                cursors[i] -= 1;
                                cursor_total -= 1;
                            }
                            None => break,
                        }
                    }

                    // Fill logs based on current cursor positions
                    // (travelling up the stack)
                    while self.logs.len() < n_visible_lines {
                        let mut next: Option<(_, _, SystemTime)> = None;
                        for i in 0..self.runner_idxs.len() {
                            if cursors[i] == lens[i] {
                                continue;
                            } // container exhausted
                            let pos = lens[i] - cursors[i] - 1;
                            let log = &runner_logs[self.runner_idxs[i]][pos];

                            match next {
                                None => {
                                    next = Some((i, pos, log.0));
                                }
                                Some((_, _, t)) => {
                                    if log.0 > t {
                                        // if dates match, prefer lower log idx
                                        next = Some((i, pos, log.0));
                                    }
                                }
                            }
                        }

                        match next {
                            Some((i, pos, _)) => {
                                // Save this position for next time
                                if cursor_total == n_lines_after {
                                    self.cursors.copy_from_slice(&cursors);
                                }

                                if cursor_total >= n_lines_after {
                                    self.logs.push(ScrollStateLog {
                                        runner_idx: self.runner_idxs[i],
                                        log_pos: pos,
                                    });
                                }

                                cursors[i] += 1;
                                cursor_total += 1;
                            }
                            None => break,
                        }
                    }
                    self.logs.reverse();
                }
                widget::scrollable::Anchor::Start => {
                    // Zipper merge of logs, ordered by log time

                    // Rewind cursors if they're ahead
                    // (travelling up the stack)
                    while cursor_total > n_lines_before {
                        let mut next: Option<(_, SystemTime)> = None;
                        for i in 0..self.runner_idxs.len() {
                            if cursors[i] == 0 {
                                continue;
                            } // cursor at start
                            let pos = cursors[i] - 1;
                            let log = &runner_logs[self.runner_idxs[i]][pos];

                            match next {
                                None => {
                                    next = Some((i, log.0));
                                }
                                Some((_, t)) => {
                                    if log.0 > t {
                                        // prefer lower log
                                        next = Some((i, log.0));
                                    }
                                }
                            }
                        }

                        match next {
                            Some((i, _)) => {
                                cursors[i] -= 1;
                                cursor_total -= 1;
                            }
                            None => break,
                        }
                    }

                    // Fill logs based on current cursor positions
                    // (travelling down the stack)
                    while self.logs.len() < n_visible_lines {
                        let mut next: Option<(_, _, SystemTime)> = None;
                        for i in (0..self.runner_idxs.len()).rev() {
                            if cursors[i] == lens[i] {
                                continue;
                            } // container exhausted
                            let pos = cursors[i];
                            let log = &runner_logs[self.runner_idxs[i]][pos];

                            match next {
                                None => {
                                    next = Some((i, pos, log.0));
                                }
                                Some((_, _, t)) => {
                                    if log.0 <= t {
                                        next = Some((i, pos, log.0));
                                    }
                                }
                            }
                        }

                        match next {
                            Some((i, pos, _)) => {
                                // Save this position for next time
                                if cursor_total == n_lines_before {
                                    self.cursors.copy_from_slice(&cursors);
                                }

                                if cursor_total >= n_lines_before {
                                    self.logs.push(ScrollStateLog {
                                        runner_idx: self.runner_idxs[i],
                                        log_pos: pos,
                                    });
                                }

                                cursors[i] += 1;
                                cursor_total += 1;
                            }
                            None => break,
                        }
                    }
                }
            }

            iced::Task::none()
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use itertools::iproduct;

        #[test]
        fn logs_are_ordered() {
            #[derive(Debug)]
            enum CursorPos {
                Start,
                Middle,
                End,
            }

            let test_anchors = &[
                widget::scrollable::Anchor::Start,
                widget::scrollable::Anchor::End,
            ];
            let test_cursors = &[CursorPos::Start, CursorPos::Middle, CursorPos::End];

            for (anchor_y, cursor_pos) in iproduct!(test_anchors, test_cursors) {
                let mut scroll_state = ScrollState::new();
                assert_eq!(scroll_state.logs.len(), 0);

                println!("test: {:?}", (anchor_y, cursor_pos));

                use rand::{SeedableRng, rngs::StdRng, seq::IndexedRandom};
                let mut rng = StdRng::seed_from_u64(99);
                let runner_idxs = [0, 1];
                let logs = (0..1000)
                    .map(|i| {
                        (
                            *runner_idxs.choose(&mut rng).unwrap() as usize,
                            format!("msg {i}\n"),
                        )
                    })
                    .collect::<Vec<_>>();

                let mut runner_logs = vec![Vec::new(); runner_idxs.len()];
                for i in 0..logs.len() {
                    let log = &logs[i];
                    runner_logs[log.0].push((SystemTime::now(), IO::Stderr(log.1.clone())));
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }

                let _ = scroll_state.set_runner_idxs(runner_idxs.iter().map(|v| *v));

                scroll_state.anchor_y = *anchor_y;

                match cursor_pos {
                    CursorPos::Start => {
                        for i in 0..scroll_state.cursors.len() {
                            scroll_state.cursors[i] = 0;
                        }
                    }
                    CursorPos::Middle => {
                        for i in 0..scroll_state.cursors.len() {
                            scroll_state.cursors[i] = 0;
                        }
                        for i in 0..logs.len() / 2 {
                            scroll_state.cursors[logs[i].0] += 1;
                        }
                    }
                    CursorPos::End => {
                        for i in 0..scroll_state.cursors.len() {
                            scroll_state.cursors[i] = runner_logs[i].len();
                        }
                    }
                }

                let _ = scroll_state.update_logs(&runner_logs);

                assert_eq!(scroll_state.logs.len(), 1000);
                for i in 0..scroll_state.logs.len() {
                    let target_log = &logs[i];
                    assert_eq!(scroll_state.logs[i].runner_idx, target_log.0);
                    assert_eq!(
                        runner_logs[scroll_state.logs[i].runner_idx][scroll_state.logs[i].log_pos]
                            .1,
                        IO::Stderr(format!("msg {i}\n"))
                    );
                }
            }
        }
    }
}
