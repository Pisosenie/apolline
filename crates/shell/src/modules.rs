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
            let mut command = tokio::process::Command::new("sh");
            command
                .args(["-c", &grouped(&module.command)])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .process_group(0)
                .kill_on_drop(true);
            crate::child::tie(command.as_std_mut(), libc::SIGTERM);
            match command.spawn() {
                Ok(mut child) => {
                    let mut group = crate::child::Group::new(child.id());
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
                    group.let_go();
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

// sh passes its own end on to the whole group, pipelines and loops included,
// since the death signal from the shell reaches only sh itself.
fn grouped(command: &str) -> String {
    format!("trap 'trap - TERM; kill 0' TERM\n{{ {command}\n}} &\nwait")
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
