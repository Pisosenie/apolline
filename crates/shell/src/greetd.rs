use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use serde_json::{Value, json};

pub fn login(socket: &str, user: &str, password: &str, command: &[String]) -> Result<(), String> {
    let mut stream = UnixStream::connect(socket).map_err(|err| format!("greetd: {err}"))?;
    let result = converse(&mut stream, user, password, command);
    if result.is_err() {
        let _ = send(&mut stream, &json!({ "type": "cancel_session" }));
    }
    result
}

fn converse(
    stream: &mut UnixStream,
    user: &str,
    password: &str,
    command: &[String],
) -> Result<(), String> {
    let mut reply = send(
        stream,
        &json!({ "type": "create_session", "username": user }),
    )?;
    let mut answered = false;
    loop {
        match reply["type"].as_str() {
            Some("auth_message") => {
                let asked = reply["auth_message"].as_str().unwrap_or("Login");
                let response = match reply["auth_message_type"].as_str() {
                    Some("secret") if !answered => {
                        answered = true;
                        Some(password)
                    }
                    Some("visible") if asks_for_login(asked) => Some(user),
                    // A visible question, a code say, never gets the password.
                    Some("secret" | "visible") => {
                        return Err(format!("{asked} asks for more than a password"));
                    }
                    _ => None,
                };
                reply = send(
                    stream,
                    &json!({ "type": "post_auth_message_response", "response": response }),
                )?;
            }
            Some("success") => break,
            Some("error") => return Err(said(&reply)),
            _ => return Err("greetd answered something unexpected".into()),
        }
    }
    let started = send(
        stream,
        &json!({ "type": "start_session", "cmd": command, "env": [] }),
    )?;
    match started["type"].as_str() {
        Some("success") => Ok(()),
        _ => Err(said(&started)),
    }
}

fn asks_for_login(prompt: &str) -> bool {
    let prompt = prompt.to_lowercase();
    prompt.starts_with("login") || prompt.contains("user")
}

fn said(reply: &Value) -> String {
    match reply["error_type"].as_str() {
        Some("auth_error") => "Wrong password".into(),
        _ => reply["description"]
            .as_str()
            .unwrap_or("greetd refused")
            .to_owned(),
    }
}

fn send(stream: &mut UnixStream, message: &Value) -> Result<Value, String> {
    let body = message.to_string();
    let length = u32::try_from(body.len()).map_err(|_| "message too long")?;
    stream
        .write_all(&length.to_ne_bytes())
        .and_then(|()| stream.write_all(body.as_bytes()))
        .map_err(|err| format!("greetd: {err}"))?;
    let mut size = [0u8; 4];
    stream
        .read_exact(&mut size)
        .map_err(|err| format!("greetd: {err}"))?;
    let mut reply = vec![0u8; u32::from_ne_bytes(size) as usize];
    stream
        .read_exact(&mut reply)
        .map_err(|err| format!("greetd: {err}"))?;
    serde_json::from_slice(&reply).map_err(|err| format!("greetd: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    fn serve(listener: UnixListener, replies: Vec<Value>) -> std::thread::JoinHandle<Vec<Value>> {
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut seen = Vec::new();
            for reply in replies {
                let mut size = [0u8; 4];
                if stream.read_exact(&mut size).is_err() {
                    break;
                }
                let mut body = vec![0u8; u32::from_ne_bytes(size) as usize];
                stream.read_exact(&mut body).unwrap();
                seen.push(serde_json::from_slice::<Value>(&body).unwrap());
                let out = reply.to_string();
                stream.write_all(&(out.len() as u32).to_ne_bytes()).unwrap();
                stream.write_all(out.as_bytes()).unwrap();
            }
            seen
        })
    }

    fn socket(name: &str) -> (String, UnixListener) {
        let path =
            std::env::temp_dir().join(format!("apolline-greetd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        (path.display().to_string(), listener)
    }

    #[test]
    fn a_password_opens_the_session() {
        let (path, listener) = socket("ok");
        let server = serve(
            listener,
            vec![
                json!({"type": "auth_message", "auth_message_type": "secret", "auth_message": "Password:"}),
                json!({"type": "success"}),
                json!({"type": "success"}),
            ],
        );
        let command = vec!["niri-session".to_owned()];
        assert_eq!(login(&path, "dzorin", "polina", &command), Ok(()));
        let seen = server.join().unwrap();
        assert_eq!(seen[0]["username"], "dzorin");
        assert_eq!(seen[1]["response"], "polina");
        assert_eq!(seen[2]["cmd"][0], "niri-session");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_visible_question_never_gets_the_password() {
        let (path, listener) = socket("code");
        let server = serve(
            listener,
            vec![
                json!({"type": "auth_message", "auth_message_type": "visible", "auth_message": "login:"}),
                json!({"type": "auth_message", "auth_message_type": "visible", "auth_message": "Verification code:"}),
                json!({"type": "success"}),
            ],
        );
        let result = login(&path, "dzorin", "polina", &["niri-session".to_owned()]);
        assert_eq!(
            result,
            Err("Verification code: asks for more than a password".into())
        );
        let seen = server.join().unwrap();
        assert_eq!(seen[1]["response"], "dzorin");
        assert_eq!(seen[2]["type"], "cancel_session");
        assert!(!seen.iter().any(|said| said["response"] == "polina"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_wrong_password_says_so_and_cancels() {
        let (path, listener) = socket("bad");
        let server = serve(
            listener,
            vec![
                json!({"type": "auth_message", "auth_message_type": "secret", "auth_message": "Password:"}),
                json!({"type": "error", "error_type": "auth_error", "description": "pam_authenticate: AUTH_ERR"}),
                json!({"type": "success"}),
            ],
        );
        let result = login(&path, "dzorin", "nope", &["niri-session".to_owned()]);
        assert_eq!(result, Err("Wrong password".into()));
        let seen = server.join().unwrap();
        assert_eq!(seen[2]["type"], "cancel_session");
        let _ = std::fs::remove_file(path);
    }
}
