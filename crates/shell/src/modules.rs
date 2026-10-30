use std::process::Stdio;
use std::time::Duration;

use apolline_config::Module;
use iced::Subscription;
use iced::futures::{SinkExt, Stream};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::shell::Message;

const LONGEST: usize = 48;

pub fn subscription(modules: &[Module]) -> Subscription<Message> {
    Subscription::batch(
        modules
            .iter()
            .cloned()
            .enumerate()
            .map(|module| Subscription::run_with(module, run)),
    )
}

fn run((index, module): &(usize, Module)) -> impl Stream<Item = Message> + use<> {
    let (index, module) = (*index, module.clone());
    iced::stream::channel(4, async move |mut output| {
        loop {
            let child = tokio::process::Command::new("sh")
                .args(["-c", &module.command])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn();
            match child {
                Ok(mut child) => {
                    if let Some(stdout) = child.stdout.take() {
                        let mut lines = BufReader::new(stdout).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            if output
                                .send(Message::Module(index, tidy(&line)))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                    let _ = child.wait().await;
                }
                Err(err) => {
                    eprintln!("apolline: module {}: {err}", module.command);
                }
            }
            let wait = module.every.map_or(30, u64::from);
            tokio::time::sleep(Duration::from_secs(wait)).await;
        }
    })
}

pub fn tidy(line: &str) -> String {
    let line = line.trim();
    match line.char_indices().nth(LONGEST) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_lines_are_cut_with_an_ellipsis() {
        assert_eq!(tidy("  12°C  "), "12°C");
        let long = "ж".repeat(60);
        assert_eq!(tidy(&long).chars().count(), LONGEST + 1);
        assert!(tidy(&long).ends_with('…'));
    }
}
