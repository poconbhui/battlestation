mod app;
mod icon;
mod runner;

use app::App;
use runner::Runner;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the battlestation UI (default)
    UI {
        #[arg(short,long)]
        config: String,
    },
    /// Run a command, ensure children are cleaned up in SIGTERM
    Run {
        /// Run command in a subshell
        #[arg(short)]
        command_string: String,
    },
}

#[derive(serde::Deserialize)]
struct Config {
    runners: Vec<RunnerConfig>,
}

#[derive(serde::Deserialize)]
struct RunnerConfig {
    name: String,
    script: String,
}

impl From<RunnerConfig> for runner::Runner {
    fn from(rc: RunnerConfig) -> runner::Runner {
        Runner::new(rc.name, rc.script)
    }
}

fn main() -> std::process::ExitCode {
    let args = Args::parse();

    match args.command {
        Command::UI { config } => {
            let config = match std::fs::read_to_string(&config) {
                Ok(fp) => { fp },
                Err(e) => {
                    use clap::CommandFactory;
                    Args::command().error(
                        clap::error::ErrorKind::ValueValidation,
                        format!("Error opening config file {config}: {e}")
                    ).exit();
                    return std::process::ExitCode::FAILURE;
                }
            };

            let config = match serde_json::from_str::<Config>(&config) {
                Ok(config) => { config },
                Err(e) => {
                    println!("Error parsing json: {e}");
                    return std::process::ExitCode::FAILURE;
                }
            };

            let res = iced::application("Battlestation", App::update, App::view).run_with(|| {
                let app = App::new(
                    config.runners
                        .into_iter()
                        .map(Into::into)
                        .collect()
                );

                let load_font = iced::font::load(icon::ICON_FONT_BYTES).discard();

                (app, load_font)
            });

            if let Err(e) = res {
                println!("Exiting with error: {e:?}");
                std::process::ExitCode::FAILURE
            } else {
                std::process::ExitCode::SUCCESS
            }
        }
        Command::Run { command_string } => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            let res = rt.block_on(async {
                let mut command = tokio::process::Command::new("/bin/bash");
                command.arg("-c");
                command.arg(command_string);

                // Get sudo to make gui prompt for password
                command.env("SUDO_ASKPASS", "/Users/poconbhui/prog/battlestation/_askpass.sh");

                // Make new session, disconnecting tty
                let setsid_res = unsafe { libc::setsid() };

                // Set PGID of command to child_pid, so we can use killpg
                command.process_group(0);

                let mut child = command.spawn().unwrap();
                let child_pid = child.id().unwrap() as i32;

                // Check if parent died by checking if this process has been
                // reparented
                let parent_died = async {
                    let prev_ppid = unsafe { libc::getppid() };
                    loop {
                        let current_ppid = unsafe { libc::getppid() };
                        if current_ppid != prev_ppid {
                            println!("Parent died");
                            return;
                        }

                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                };

                let signal_listener = async |raw_signal| {
                    let mut listener = tokio::signal::unix::signal(
                        tokio::signal::unix::SignalKind::from_raw(raw_signal),
                    )
                    .unwrap();
                    listener.recv().await
                };

                tokio::select! {
                    _ = child.wait() => {},
                    // Kill our child when our parent dies
                    _ = parent_died => {
                        println!("Parent died, cleaning up");
                        unsafe { libc::kill(child_pid, libc::SIGTERM) };
                    },
                    // Forward signals
                    _ = signal_listener(libc::SIGINT) => {
                        unsafe { libc::kill(child_pid, libc::SIGINT) };
                    },
                    _ = signal_listener(libc::SIGTERM) => {
                        unsafe { libc::kill(child_pid, libc::SIGTERM) };
                    },
                    _ = signal_listener(libc::SIGPIPE) => {
                        unsafe { libc::kill(child_pid, libc::SIGPIPE) };
                    }
                };

                // Child has finished, or been sent a deadly signal.
                // Wait a bit, and kill it if it doesn't finish
                tokio::select! {
                    _ = child.wait() => {},
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(5000)) => {
                        unsafe { libc::kill(child_pid, libc::SIGKILL) };
                    }
                }

                // Child has finished, or been send a very deadly signal.
                let child_res = child.wait().await;

                // Child is dead, cleanup any stragglers
                unsafe { libc::killpg(child_pid, libc::SIGTERM) };

                if let Ok(child_res) = child_res {
                    if child_res.success() {
                        std::process::ExitCode::SUCCESS
                    } else {
                        println!("Child exited with error: {child_res:?}");
                        std::process::ExitCode::FAILURE
                    }
                } else {
                    println!("Error getting child result: {child_res:?}");
                    std::process::ExitCode::FAILURE
                }
            });

            res
        }
    }
}
