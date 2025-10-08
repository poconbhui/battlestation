use crate::icon;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;

pub struct Runner {
    pub name: String,
    script: String,
    forever: bool,
    status: Status,
    stdout_activity: activity::Activity,
    stderr_activity: activity::Activity,
    pub show_logs: bool
}

enum Status {
    Off,
    Running {
        start_time: std::time::SystemTime,
        stdin_tx: mpsc::Sender<String>,
        kill_tx: Option<oneshot::Sender<()>>,
    },
    Completed {
        status: i32,
        start_time: std::time::SystemTime,
        _end_time: std::time::SystemTime,
    },
}

#[derive(Debug, Clone)]
pub enum ActivityLight {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone)]
pub enum Message {
    ScriptRun,
    ScriptKill {
        start_time: std::time::SystemTime,
    },
    ScriptComplete {
        status: i32,
        start_time: std::time::SystemTime,
        end_time: std::time::SystemTime,
    },
    ScriptClearStatus {
        start_time: std::time::SystemTime,
    },

    _Stdin(String),
    Stdout(String),
    Stderr(String),

    Activity(ActivityLight, activity::Message),

    SetShowLogs(bool),
    SetForever(bool),
}

impl Runner {
    pub fn new(name: String, script: String) -> Runner {
        Runner {
            name,
            script,
            forever: false,
            status: Status::Off,
            stdout_activity: activity::Activity::new(iced::Color::from_rgb(0.0, 1.0, 0.0)),
            stderr_activity: activity::Activity::new(iced::Color::from_rgb(1.0, 1.0, 0.0)),
            show_logs: false,
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        use iced::widget;

        let run_button = match self.status {
            Status::Off => {
                widget::button(icon::to_text(icon::Nerd::PlayOne))
                    .on_press(Message::ScriptRun)
            }
            Status::Running { start_time, .. } => {
                widget::button(icon::to_text(icon::Nerd::Stop))
                    .on_press(Message::ScriptKill { start_time })
            }
            Status::Completed { status, .. } => {
                widget::button(widget::text(status.to_string()))
                    .on_press(Message::ScriptRun)
                    .style(if status == 0 {
                        widget::button::success
                    } else {
                        widget::button::danger
                    })
            }
        };

        let activity_stdout = self.stdout_activity.view().map(|msg| Message::Activity(ActivityLight::Stdout, msg));
        let activity_stderr = self.stderr_activity.view().map(|msg| Message::Activity(ActivityLight::Stderr, msg));
        let activity = widget::column![activity_stdout, activity_stderr];

        let forever_button = if self.forever {
            widget::button(crate::icon::to_text(crate::icon::Nerd::RepeatOne))
                .on_press(Message::SetForever(false))
                .style(widget::button::success)
        } else {
            widget::button(crate::icon::to_text(crate::icon::Nerd::RepeatOne))
                .on_press(Message::SetForever(true))
                .style(widget::button::secondary)
        };

        let logs_button = if self.show_logs {
            widget::button(crate::icon::to_text(crate::icon::Nerd::TextBoxOutline))
                .on_press(Message::SetShowLogs(false))
                .style(widget::button::success)
        } else {
            widget::button(crate::icon::to_text(crate::icon::Nerd::TextBoxOutline))
                .on_press(Message::SetShowLogs(true))
                .style(widget::button::secondary)
        };

        widget::column![
            widget::text(&self.name),
            widget::row![run_button, activity, forever_button, logs_button]
                .align_y(iced::Alignment::Center)
                .spacing(5),
        ]
        .into()
    }

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::ScriptClearStatus {
                start_time: target_start_time,
                ..
            } => {
                match self.status {
                    Status::Completed {
                        start_time: status_start_time,
                        ..
                    } => {
                        if status_start_time == target_start_time {
                            self.status = Status::Off;
                            if self.forever {
                                iced::Task::done(Message::ScriptRun)
                            } else {
                                iced::Task::none()
                            }
                        } else {
                            println!("[{}][<ClearStatus>] start_time mismatched", self.name);
                            iced::Task::none()
                        }
                    }
                    _ => {
                        println!(
                            "[{}][<ClearStatus>] script not in completed state",
                            self.name
                        );
                        iced::Task::none()
                    }
                }
            }

            Message::ScriptRun => match self.status {
                Status::Off | Status::Completed{..}=> {
                    println!("[{}][<Run>] Running task", self.name);

                    let (stdin_tx, stdin_rx) = mpsc::channel(1024);
                    let (stdout_tx, stdout_rx) = mpsc::channel(1024);
                    let (stderr_tx, stderr_rx) = mpsc::channel(1024);
                    let (kill_tx, kill_rx) = oneshot::channel();

                    let start_time = std::time::SystemTime::now();
                    self.status = Status::Running {
                        start_time: start_time.clone(),
                        stdin_tx,
                        kill_tx: Some(kill_tx),
                    };
                    let stdout_stream = ReceiverStream::new(stdout_rx);
                    let stderr_stream = ReceiverStream::new(stderr_rx);

                    iced::Task::batch([
                        iced::Task::perform(
                            Runner::exec(
                                self.name.clone(),
                                self.script.clone(),
                                stdin_rx,
                                stdout_tx,
                                stderr_tx,
                                kill_rx,
                            ),
                            move |status| {
                                Message::ScriptComplete {
                                    status,
                                    start_time,
                                    end_time: std::time::SystemTime::now(),
                                }
                            },
                        ),
                        iced::Task::run(stdout_stream, |s| Message::Stdout(s)),
                        iced::Task::run(stderr_stream, |s| Message::Stderr(s)),
                    ])
                }
                _ => {
                    println!("[{}][<Run>] already running", self.name);
                    iced::Task::none()
                }
            }

            Message::ScriptKill {
                start_time: target_start_time,
            } => match &mut self.status {
                Status::Running {
                    start_time,
                    kill_tx,
                    ..
                } => {
                    if *start_time == target_start_time {
                        if let Some(kill_tx) = kill_tx.take() {
                            let _ = kill_tx.send(());
                        }
                    }
                    iced::Task::none()
                }
                _ => {
                    println!("[{}][<Kill>] not running", self.name);
                    iced::Task::none()
                }
            }

            Message::ScriptComplete {
                status,
                start_time,
                end_time,
            } => {
                println!("[{}][<Complete>] status {status}", self.name);

                self.status = Status::Completed {
                    status,
                    start_time,
                    _end_time: end_time,
                };

                let start_time = start_time.clone();
                iced::Task::future(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                    Message::ScriptClearStatus { start_time }
                })
            }

            Message::_Stdin(s) => match &self.status {
                Status::Running { stdin_tx, .. } => {
                    let name = self.name.clone();
                    let stdin_tx = stdin_tx.clone();
                    iced::Task::future(async move {
                        if let Err(err) = stdin_tx.send(s).await {
                            println!("[{name}][<Stdin>] {err:?}");
                        }
                    })
                    .discard()
                }
                _ => {
                    println!("[{}][<Stdin>] task not running", self.name);
                    iced::Task::none()
                }
            }
            Message::Stdout(s) => {
                println!("[{}][>] {s}", self.name);

                self.stdout_activity.trigger()
                    .map(|msg| Message::Activity(ActivityLight::Stdout, msg))
            }
            Message::Stderr(s) => {
                println!("[{}][!] {s}", self.name);

                self.stderr_activity.trigger()
                    .map(|msg| Message::Activity(ActivityLight::Stderr, msg))

            }

            Message::Activity(ActivityLight::Stdout, message) => {
                self.stdout_activity.update(message)
                    .map(|msg| Message::Activity(ActivityLight::Stdout, msg))
            }
            Message::Activity(ActivityLight::Stderr, message) => {
                self.stderr_activity.update(message)
                    .map(|msg| Message::Activity(ActivityLight::Stderr, msg))
            }

            Message::SetShowLogs(v) => {
                self.show_logs = v;
                iced::Task::none()
            }
            Message::SetForever(v) => {
                self.forever = v;
                iced::Task::none()
            }
        }
    }

    async fn exec(
        name: String,
        script: String,
        _stdin_rx: mpsc::Receiver<String>,
        stdout_tx: mpsc::Sender<String>,
        stderr_tx: mpsc::Sender<String>,
        kill_rx: oneshot::Receiver<()>,
    ) -> i32 {
        use tokio::io::AsyncReadExt;

        println!("[{name}] ---- BEGIN ----");

        let current_exe = match std::env::current_exe() {
            Ok(current_exe) => { current_exe },
            Err(err) => {
                let err = format!("Unable to find current exe: {err:?}");
                println!("[{name}][!] {err}");
                let _ = stderr_tx.send(err).await;
                return 99;
            }
        };

        let mut command = tokio::process::Command::new(current_exe);
        command.arg("run");
        command.arg("-c");
        command.arg(script);

        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());
        command.stdin(std::process::Stdio::piped());

        let mut child = command.spawn().unwrap();
        let child_pid = child.id().unwrap() as i32;

        let Some(mut stdout) = child.stdout.take() else {
            println!("[{name}] Error getting stdout");
            return 99;
        };
        let Some(mut stderr) = child.stderr.take() else {
            println!("[{name}] Error getting stderr");
            return 99;
        };
        let Some(mut stdin) = child.stdin.take() else {
            println!("[{name}] Error getting stdin");
            return 99;
        };

        use tokio::io::AsyncWriteExt;
        let _ = stdin.shutdown().await;

        let _name = name.clone();
        let reading_stdout_handle = tokio::task::spawn( async move {
            let name = _name;
            let mut stdout_open = true;
            let mut stderr_open = true;
            let mut stdout_buf = [0u8; 1024];
            let mut stderr_buf = [0u8; 1024];
            loop {
                if !stdout_open && !stderr_open {
                    break;
                }
                tokio::select! {
                    n = stdout.read(&mut stdout_buf), if stdout_open => {
                        match n {
                            Ok(0) => {
                                stdout_open = false;
                            },
                            Ok(n) => {
                                let s = String::from_utf8_lossy(&stdout_buf[..n]).into_owned();
                                let _ = stdout_tx.send(s).await;
                                for i in 0..n {
                                    stdout_buf[i] = 0;
                                }
                            },
                            Err(e) => {
                                println!("[{name}][>][!] io error: {e:?}");
                            }
                        }
                    },
                    n = stderr.read(&mut stderr_buf), if stderr_open => {
                        match n {
                            Ok(0) => {
                                stderr_open = false;
                            },
                            Ok(n) => {
                                let s = String::from_utf8_lossy(&stderr_buf[..n]).into_owned();
                                let _ = stderr_tx.send(s).await;
                                for i in 0..n {
                                    stdout_buf[i] = 0;
                                }
                            },
                            Err(e) => {
                                println!("[{name}][!][!] io error: {e:?}");
                            }
                        }
                    },
                }
            }
        });

        tokio::select! {
            _ = child.wait() => {},
            _ = kill_rx => {
                unsafe { libc::kill(child_pid, libc::SIGTERM) };
            }
        }

        let res = child.wait().await;
        let _ = reading_stdout_handle.await;
        println!("[{name}] res {res:?}");

        println!("[{name}] ---- END ----");

        if let Ok(res) = res {
            if res.success() {
                0
            } else {
                1
            }
        } else {
            1
        }
    }
}

mod activity {
    pub struct Activity {
        state: State,
        color: iced::Color,
    }

    enum State {
        On(std::time::SystemTime),
        Off(std::time::SystemTime),
    }

    #[derive(Debug, Clone)]
    pub enum Message {
        Trigger,
        Clear(std::time::SystemTime),
    }

    impl Activity {
        pub fn new(color: iced::Color) -> Activity {
            Activity {
                state: State::Off(std::time::UNIX_EPOCH),
                color,
            }
        }

        pub fn view(&self) -> iced::Element<'_, Message> {
            let icon = match self.state {
                State::On(_)  => crate::icon::Nerd::SquareRounded,
                State::Off(_) => crate::icon::Nerd::SquareRoundedOutline,
            };

            crate::icon::to_text(icon)
                .color(self.color)
                .into()
        }

        pub fn trigger(&mut self) -> iced::Task<Message> {
            self.update(Message::Trigger)
        }

        pub fn update(&mut self, message: Message) -> iced::Task<Message> {
            let on_len = std::time::Duration::from_millis(100);
            let off_len = std::time::Duration::from_millis(50);
            match message {
                Message::Trigger => {
                    match &mut self.state {
                        State::Off(t) => {
                            if std::time::SystemTime::now() >= *t + off_len {
                                let changed_at = std::time::SystemTime::now();
                                self.state = State::On(changed_at.clone());

                                iced::Task::future(async move {
                                    tokio::time::sleep(on_len).await;
                                    Message::Clear(changed_at)
                                })
                            } else {
                                iced::Task::none()
                            }
                        },
                        _ => iced::Task::none()
                    }
                },
                Message::Clear(target_t) => {
                    match &mut self.state {
                        State::On(t) => {
                            if target_t == *t {
                                self.state = State::Off(std::time::SystemTime::now());
                            }
                            iced::Task::none()
                        },
                        _ => iced::Task::none()
                    }
                }
            }
        }
    }
}
