use std::sync::mpsc::Sender;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use zeroize::Zeroizing;

#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    pub text: String,
    pub shown: bool,
}

#[derive(Clone, Debug)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub enum Step {
    // PAM wants more than the password, a one-time code say.
    Ask(Question, Sender<Zeroizing<String>>),
    Note(String),
    Done(Result<(), String>),
}

pub fn check(login: String, password: Zeroizing<String>) -> impl Stream<Item = Step> {
    iced::stream::channel(4, async move |mut output| {
        let (send, mut steps) = mpsc::unbounded();
        let talk = tokio::task::spawn_blocking(move || {
            pam::authenticate(&service(), &login, password, send)
        });
        while let Some(step) = steps.next().await {
            if output.send(step).await.is_err() {
                return;
            }
        }
        let done = talk
            .await
            .map_err(|err| err.to_string())
            .and_then(|done| done);
        let _ = output.send(Step::Done(done)).await;
    })
}

fn service() -> String {
    std::env::var("APOLLINE_PAM_SERVICE").unwrap_or_else(|_| {
        if std::path::Path::new("/etc/pam.d/apolline").exists() {
            "apolline".to_owned()
        } else {
            "login".to_owned()
        }
    })
}

#[cfg(target_os = "linux")]
mod pam {
    use std::ffi::{CStr, CString, c_char, c_int, c_void};
    use std::ptr;
    use std::sync::mpsc;
    use std::time::Duration;

    use ::pam::ffi;
    use iced::futures::channel::mpsc::UnboundedSender;
    use zeroize::{Zeroize, Zeroizing};

    use super::{Question, Step};

    // Long enough to find a phone for a code, short enough not to hang a forgotten lock.
    const WAIT: Duration = Duration::from_secs(120);

    struct Talk {
        password: Option<Zeroizing<String>>,
        steps: UnboundedSender<Step>,
        // What PAM said went wrong, to show if it fails.
        errors: Vec<String>,
    }

    impl Talk {
        fn ask(&self, question: Question) -> Option<Zeroizing<String>> {
            let (answer, answers) = mpsc::channel();
            self.steps
                .unbounded_send(Step::Ask(question, answer))
                .ok()?;
            answers.recv_timeout(WAIT).ok()
        }

        // The typed password answers the first hidden prompt; anything else is asked on screen.
        fn answer(&mut self, style: c_int, text: String) -> Result<Option<Zeroizing<String>>, ()> {
            match style {
                ffi::PAM_PROMPT_ECHO_OFF => self
                    .password
                    .take()
                    .or_else(|| self.ask(Question { text, shown: false }))
                    .map(Some)
                    .ok_or(()),
                ffi::PAM_PROMPT_ECHO_ON => {
                    self.ask(Question { text, shown: true }).map(Some).ok_or(())
                }
                ffi::PAM_TEXT_INFO => {
                    let _ = self.steps.unbounded_send(Step::Note(text));
                    Ok(None)
                }
                ffi::PAM_ERROR_MSG => {
                    let _ = self.steps.unbounded_send(Step::Note(text.clone()));
                    self.errors.push(text);
                    Ok(None)
                }
                _ => Err(()),
            }
        }
    }

    pub fn authenticate(
        service: &str,
        login: &str,
        password: Zeroizing<String>,
        steps: UnboundedSender<Step>,
    ) -> Result<(), String> {
        let service = CString::new(service).map_err(|err| err.to_string())?;
        let user = CString::new(login).map_err(|err| err.to_string())?;
        let mut talk = Box::new(Talk {
            password: Some(password),
            steps,
            errors: Vec::new(),
        });
        let conversation = ffi::pam_conv {
            conv: Some(converse),
            appdata_ptr: ptr::from_mut(&mut *talk).cast(),
        };
        let mut handle = ptr::null_mut();
        // SAFETY: every pointer outlives the transaction, which pam_end closes below.
        let started =
            unsafe { ffi::pam_start(service.as_ptr(), user.as_ptr(), &conversation, &mut handle) };
        if started != ffi::PAM_SUCCESS || handle.is_null() {
            return Err(refusal(started));
        }
        // SAFETY: the handle is the one pam_start just opened.
        let code = unsafe { transact(handle) };
        // SAFETY: as above, and nothing touches the handle after this.
        unsafe { ffi::pam_end(handle, code) };
        if code == ffi::PAM_SUCCESS {
            return Ok(());
        }
        Err(talk.errors.pop().unwrap_or_else(|| refusal(code)))
    }

    unsafe fn transact(handle: *mut ffi::pam_handle_t) -> c_int {
        // SAFETY: the caller's handle is open.
        unsafe {
            let code = ffi::pam_authenticate(handle, 0);
            if code != ffi::PAM_SUCCESS {
                return code;
            }
            // An expired password is for logging in to change; it doesn't lock the owner out.
            let code = ffi::pam_acct_mgmt(handle, 0);
            if code != ffi::PAM_SUCCESS && code != ffi::PAM_NEW_AUTHTOK_REQD {
                return code;
            }
            // A lock refreshes what logging in set up, Kerberos tickets and the like; it never
            // deletes them.
            ffi::pam_setcred(handle, ffi::PAM_REFRESH_CRED);
        }
        ffi::PAM_SUCCESS
    }

    unsafe extern "C" fn converse(
        count: c_int,
        messages: *mut *const ffi::pam_message,
        replies: *mut *mut ffi::pam_response,
        data: *mut c_void,
    ) -> c_int {
        let Ok(count) = usize::try_from(count) else {
            return ffi::PAM_CONV_ERR;
        };
        if count == 0 || messages.is_null() || replies.is_null() || data.is_null() {
            return ffi::PAM_CONV_ERR;
        }
        // SAFETY: data is the Talk given to pam_start, alive for the whole transaction.
        let talk = unsafe { &mut *data.cast::<Talk>() };
        // SAFETY: PAM frees the replies with free(), so they come from calloc.
        let out: *mut ffi::pam_response =
            unsafe { libc::calloc(count, size_of::<ffi::pam_response>()) }.cast();
        if out.is_null() {
            return ffi::PAM_BUF_ERR;
        }
        for i in 0..count {
            // SAFETY: Linux-PAM passes count pointers to messages.
            let message = unsafe { &**messages.add(i) };
            let text = if message.msg.is_null() {
                String::new()
            } else {
                // SAFETY: a message's text is a C string.
                unsafe { CStr::from_ptr(message.msg) }
                    .to_string_lossy()
                    .into_owned()
            };
            let reply = match talk.answer(message.msg_style, text) {
                Ok(None) => continue,
                Ok(Some(answer)) => copy(&answer),
                Err(()) => None,
            };
            match reply {
                // SAFETY: i is within the count calloc made room for.
                Some(reply) => unsafe { (*out.add(i)).resp = reply },
                None => {
                    // SAFETY: out holds count replies, each null or from copy.
                    unsafe { discard(out, count) };
                    return ffi::PAM_CONV_ERR;
                }
            }
        }
        // SAFETY: checked non-null above.
        unsafe { *replies = out };
        ffi::PAM_SUCCESS
    }

    // A C string PAM can free, made without a copy of the secret left behind.
    fn copy(secret: &str) -> Option<*mut c_char> {
        if secret.as_bytes().contains(&0) {
            return None;
        }
        // SAFETY: malloc'd for one more byte than is written below the terminator.
        unsafe {
            let reply = libc::malloc(secret.len() + 1).cast::<u8>();
            if reply.is_null() {
                return None;
            }
            ptr::copy_nonoverlapping(secret.as_ptr(), reply, secret.len());
            *reply.add(secret.len()) = 0;
            Some(reply.cast())
        }
    }

    unsafe fn discard(out: *mut ffi::pam_response, count: usize) {
        // SAFETY: the caller's out holds count replies, each null or a C string from copy.
        unsafe {
            for i in 0..count {
                let reply = (*out.add(i)).resp;
                if !reply.is_null() {
                    std::slice::from_raw_parts_mut(reply.cast::<u8>(), libc::strlen(reply))
                        .zeroize();
                    libc::free(reply.cast());
                }
            }
            libc::free(out.cast());
        }
    }

    fn refusal(code: c_int) -> String {
        match code {
            ffi::PAM_AUTH_ERR
            | ffi::PAM_CRED_INSUFFICIENT
            | ffi::PAM_AUTHTOK_ERR
            | ffi::PAM_USER_UNKNOWN => "Wrong password".into(),
            ffi::PAM_MAXTRIES => "Too many tries, wait a little".into(),
            ffi::PAM_ACCT_EXPIRED => "Your account has expired".into(),
            other => {
                // SAFETY: Linux-PAM's pam_strerror never reads the handle.
                let reason = unsafe { CStr::from_ptr(ffi::pam_strerror(ptr::null_mut(), other)) };
                crate::i18n::fill(
                    "Can't unlock: {reason}",
                    &[("reason", &reason.to_string_lossy())],
                )
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod pam {
    use iced::futures::channel::mpsc::UnboundedSender;
    use zeroize::Zeroizing;

    pub fn authenticate(
        _: &str,
        _: &str,
        _: Zeroizing<String>,
        _: UnboundedSender<super::Step>,
    ) -> Result<(), String> {
        Err("Unlocking needs Linux".into())
    }
}
