//! The window, the tray and the four commands the page can call.
//!
//! Everything that could be wrong here has been decided somewhere else —
//! [`crate::attach`], [`crate::autostart`], [`crate::dialogs`],
//! [`crate::spawn`] — so this module is assembly. That is the whole of
//! `CLAUDE.md`'s rule as it applies to a shell: **no test may need a window**,
//! so nothing a test would want to assert is written down here.
//!
//! # What closing the window does, and what quitting does
//!
//! **D9, and `ARCHITECTURE_SPEC.md` §10.3: accidentally closing a window must
//! never end a show.** So the close button hides the window and the tray icon
//! stays; the daemon is not told anything, because nothing about it has changed.
//! Quitting from the tray closes the shell and *still* leaves the daemon
//! running, which is §10.3's default tier written out — the shell spawns
//! `prismd`, detaches it and leaves it running on close.
//!
//! Stopping the desk is a **third** item and it says so: it sends
//! `Command::Shutdown` over the local transport, which is the one
//! `docs/IPC_PROTOCOL.md` §2 gives the desktop shell, and then quits. That is
//! the tray menu §10.3 has named since S17 and had no way to perform.
//!
//! # And what happens when the desk goes without being asked — B39, S51
//!
//! A desk killed from a task manager left the icon standing, *Stop the desk*
//! looking for a process that was not there, and the next start putting a
//! second icon beside the first. So the shell **watches the guard**
//! ([`crate::attach::standing`], which has the argument for the guard rather
//! than the connection) once a second, and a shell whose desk has gone stands
//! down: the tray says so, the operator is told in a sentence that answers
//! *is the show still on*, and the shell closes. Nothing of a dead desk is left
//! in the notification area for the next start to sit beside.
//!
//! The watching is here and the deciding is not: what the poll produces is a
//! [`crate::attach::Standing`], which is decided from two numbers in a module a
//! test can call without a window.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::DialogExt;

use crate::attach::{Approach, Standing, standing, stopped_message};
use crate::autostart::{self, Report};
use crate::dialogs::{Mode, PathKind, chooser};

/// The window's label, matching `capabilities/default.json`.
const DESK: &str = "desk";

/// The tray icon's identifier, so the watcher can find it again to change what
/// it says.
const TRAY: &str = "desk";

/// The tray menu's item identifiers.
const SHOW: &str = "show";
const QUIT: &str = "quit";
const STOP: &str = "stop";

/// How often the shell looks at the guard to see whether its desk is still
/// there — **B39**.
///
/// A second, and the number is chosen from what it costs and what it buys. It
/// costs one `File::open` and one `try_lock_shared` on a file the daemon is
/// already holding, which is nothing measurable; it buys an icon that is wrong
/// for at most a second. Anything faster would be a poll for a change that
/// happens once in a session, and anything slower is long enough for an
/// operator to click *Stop the desk* on a desk that is not there.
const WATCH_EVERY: Duration = Duration::from_secs(1);

/// The tooltip while the desk is running, and after it has gone.
const RUNNING_TOOLTIP: &str = "PrismDMX — the desk is running";
const STOPPED_TOOLTIP: &str = "PrismDMX — the desk has stopped";

/// What the shell worked out before the window existed, kept for the commands
/// that need it.
struct Desk {
    /// This executable, for the start-up entry's command line.
    executable: PathBuf,
    /// The daemon's local endpoint, when it published one. `None` is a desk this
    /// shell can see over WebSocket and cannot speak to over the pipe, which is
    /// an odd configuration rather than an impossible one — and the tray's
    /// *Stop the desk* is the one thing that then cannot be done.
    local: Mutex<Option<String>>,
    /// Whether the watcher has found the desk gone — **B39**. Set once and never
    /// unset: a shell whose desk has stopped is closing, and a second verdict
    /// while the first dialogue is up would be a second dialogue.
    stood_down: Mutex<bool>,
}

impl Desk {
    /// The start-up entry's command line for this installation.
    fn command_line(&self) -> String {
        autostart::command_line(&self.executable)
    }
}

/// Runs the shell.
///
/// `found` is either the daemon to attach to or the sentence to show instead:
/// `main` has already looked, and either started one or found out why it could
/// not. **A refusal is a window too**, and that is not politeness — a release
/// build is `windows_subsystem = "windows"`, so it has no terminal at all, and a
/// shell that returned an exit code here would be a program that did nothing
/// visible whatever when the engine failed to start.
///
/// `data_dir` is where this desk's files are, and `hidden` is the `--hidden` a
/// start-up entry carries.
///
/// # Panics
///
/// If Tauri cannot build an application at all, which is a broken installation
/// rather than a state to recover from.
pub fn run(
    found: Result<Approach, String>,
    data_dir: PathBuf,
    local: Option<String>,
    hidden: bool,
) {
    let executable = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("PrismDMX.exe"));
    let desk = Desk {
        executable,
        local: Mutex::new(local),
        stood_down: Mutex::new(false),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(desk)
        .invoke_handler(tauri::generate_handler![
            choose_path,
            autostart_state,
            autostart_apply,
            set_fullscreen,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            match refusal(&found, &data_dir) {
                None => {
                    let Ok(Approach::Attach { url, token, .. }) = &found else {
                        unreachable!("`refusal` answers `None` only for an attach")
                    };
                    let Ok(Approach::Attach { pid, .. }) = &found else {
                        unreachable!("`refusal` answers `None` only for an attach")
                    };
                    open_window(&handle, url, token.as_deref(), hidden)?;
                    build_tray(&handle)?;
                    // **The switch, acted on at last** (S37 wrote it down, S29
                    // obeys it). At start, because an entry can be deleted by
                    // hand and a switch can be changed while the shell is shut.
                    reconcile_at_start(&handle, &data_dir);
                    // **B39.** From here on the shell knows which process its
                    // desk is, so it can notice when that process is no longer
                    // the one holding the guard.
                    watch_the_desk(&handle, data_dir.clone(), *pid);
                }
                Some(message) => {
                    let closing = handle.clone();
                    handle
                        .dialog()
                        .message(message)
                        .title("PrismDMX")
                        .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
                        .show(move |_| closing.exit(1));
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // **D9.** The close button hides the window; it does not end
            // anything. The tray icon is what says the desk is still there.
            if matches!(event, WindowEvent::CloseRequested { .. }) {
                let _ = window.hide();
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                }
            }
        })
        .on_menu_event(on_menu)
        .build(tauri::generate_context!())
        .expect("the shell could not be built")
        .run(|_app, event| {
            // Without this a shell whose window is hidden would exit on macOS
            // and on any platform that treats *no windows* as *nothing to do*,
            // which would take the tray icon with it.
            if let RunEvent::ExitRequested { api, code, .. } = &event
                && code.is_none()
            {
                api.prevent_exit();
            }
        });
}

/// The sentence to show instead of a window, or `None` when there is a desk to
/// attach to.
///
/// Pure, so the three ways a start can fail are asserted without one:
/// `main` could not start an engine, the engine that is running cannot be
/// reached, and — the arm that should be unreachable — a decision to spawn that
/// somehow arrived here, because `main` resolves that one before this function
/// is ever called.
fn refusal(desk: &Result<Approach, String>, data_dir: &Path) -> Option<String> {
    match desk {
        Ok(Approach::Attach { .. }) => None,
        Ok(Approach::Unreachable { pid, endpoints }) => Some(crate::attach::unreachable_message(
            *pid, endpoints, data_dir,
        )),
        Ok(Approach::Spawn) => Some(
            "PrismDMX could not work out whether its engine was running, and did not start a \
             second one."
                .to_owned(),
        ),
        Err(message) => Some(message.clone()),
    }
}

/// Builds the desk window against the daemon this shell found.
///
/// The endpoint travels as a **query parameter**, which is
/// `ui/src/ipc/endpoint.ts`'s first source and has said since S23 that *the
/// desktop shell has the file, and will pass what it read as (1)*. Nothing about
/// the interface is special-cased for the shell: what a shell supplies is what
/// an operator with two engines on a bench types into the address bar.
fn open_window(app: &AppHandle, url: &str, token: Option<&str>, hidden: bool) -> tauri::Result<()> {
    let mut query = format!("daemon={}", urlencode(url));
    if let Some(token) = token {
        query.push_str(&format!("&token={}", urlencode(token)));
    }
    let window = WebviewWindowBuilder::new(
        app,
        DESK,
        WebviewUrl::App(format!("index.html?{query}").into()),
    )
    .title("PrismDMX")
    .inner_size(1600.0, 900.0)
    .min_inner_size(1024.0, 640.0)
    .visible(!hidden)
    .build()?;
    if !hidden {
        let _ = window.set_focus();
    }
    Ok(())
}

/// Percent-encodes what a query parameter cannot carry literally.
///
/// Small enough to write: the two values that go through it are a `ws://` URL
/// and a token the daemon generated, and what has to survive is `:`, `/` and the
/// separators. A dependency for eleven characters would be a dependency for
/// eleven characters.
fn urlencode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// The tray icon and its three items.
fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, SHOW, "Show the desk window", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, STOP, "Stop the desk", true, None::<&str>)?;
    let quit = MenuItem::with_id(
        app,
        QUIT,
        "Close this window, leave the desk running",
        true,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &separator, &quit, &stop])?;

    let mut tray = TrayIconBuilder::with_id(TRAY)
        .tooltip(RUNNING_TOOLTIP)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Watches the guard, and stands the shell down when its desk has gone — **B39**.
///
/// One thread and a sleep, rather than a file-system watch: the thing being
/// watched is a *lock*, not a file's contents, and a lock is released without
/// anything being written. `prismd::lock::look` is the same reading `main` makes
/// at start, which is what makes this incapable of disagreeing with *spawn or
/// attach*.
///
/// An error reading the directory is **not** a verdict. A shell that tore its
/// icon down because a disk hiccuped would be announcing the end of a show that
/// is still running, which is the fault this is fixing, upside down.
fn watch_the_desk(app: &AppHandle, data_dir: PathBuf, attached_to: u32) {
    let handle = app.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(WATCH_EVERY);
            let Ok(presence) = prismd::lock::look(&data_dir) else {
                continue;
            };
            match standing(attached_to, &presence) {
                Standing::Holding => {}
                Standing::Gone => {
                    stand_down(&handle, attached_to, None);
                    return;
                }
                Standing::Replaced { pid } => {
                    stand_down(&handle, attached_to, Some(pid));
                    return;
                }
            }
        }
    });
}

/// Says the desk has stopped, in the tray and in a sentence, and closes.
///
/// **The tray first and the dialogue second**, and that order is the whole
/// design: the icon is what an operator looks at, so it stops claiming the desk
/// is running before anything else happens — including before the dialogue,
/// which may sit unread behind a full-screen window. *Stop the desk* is taken
/// out of the menu at the same moment, because a menu item that looks for a
/// process that is not there is exactly what B39 reports.
///
/// Then the shell **closes**, once the sentence has been acknowledged. That is
/// the second half of the entry: a dead desk's icon left in the notification
/// area is what puts a second one beside it at the next start.
fn stand_down(app: &AppHandle, attached_to: u32, replacement: Option<u32>) {
    {
        let desk: State<'_, Desk> = app.state();
        let mut stood_down = desk
            .stood_down
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *stood_down {
            return;
        }
        *stood_down = true;
    }
    if let Some(tray) = app.tray_by_id(TRAY) {
        let _ = tray.set_tooltip(Some(STOPPED_TOOLTIP));
    }
    // A hidden window would leave the dialogue with nothing to sit on, and an
    // operator with nothing to see at all.
    show_window(app);

    let closing = app.clone();
    app.dialog()
        .message(stopped_message(attached_to, replacement))
        .title("PrismDMX")
        .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
        .show(move |_| closing.exit(0));
}

/// What each tray item does.
fn on_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        SHOW => show_window(app),
        // **The shell goes, the desk stays.** §10.3's default tier: closing the
        // shell leaves `prismd` running, so the show carries on with nobody
        // watching it — which is D2 in one sentence.
        QUIT => app.exit(0),
        STOP => stop_the_desk(app),
        _ => {}
    }
}

/// Brings the window back, whether it was hidden or merely behind something.
fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(DESK) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Sends `Command::Shutdown` over the local transport, and quits afterwards.
///
/// **Asked rather than killed**, which is the whole reason the command exists:
/// §10.3's shutdown tells clients first, publishes the configured blackout or
/// hold as a frame, gives the outputs time to send it, and only then stops the
/// drivers — which is where the sACN streams carrying the last look are ended. A
/// shell that killed the process would skip every one of those.
fn stop_the_desk(app: &AppHandle) {
    let handle = app.clone();
    let (address, stood_down) = {
        let desk: State<'_, Desk> = app.state();
        let address = desk
            .local
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let stood_down = *desk
            .stood_down
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (address, stood_down)
    };
    // **B39.** The watcher has already found the desk gone, so this is not a
    // connection to attempt: it is a question with an answer, and the answer is
    // that there is nothing to stop.
    if stood_down {
        app.dialog()
            .message("The desk has already stopped. There is nothing to ask.")
            .title("PrismDMX")
            .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
            .show(|_| ());
        return;
    }
    let Some(address) = address else {
        app.dialog()
            .message(
                "This desk did not publish a local endpoint, so the shell has no way to ask it \
                 to stop. Stop it where it was started.",
            )
            .title("PrismDMX")
            .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
            .show(|_| ());
        return;
    };
    tauri::async_runtime::spawn(async move {
        match ask_to_stop(&address).await {
            Ok(()) => handle.exit(0),
            Err(error) => {
                handle
                    .dialog()
                    .message(format!("The desk could not be asked to stop: {error}"))
                    .title("PrismDMX")
                    .kind(tauri_plugin_dialog::MessageDialogKind::Error)
                    .show(|_| ());
            }
        }
    });
}

/// One connection, one command, and away.
async fn ask_to_stop(address: &str) -> Result<(), String> {
    let wire = prism_ipc::local::connect(address)
        .await
        .map_err(|error| error.to_string())?;
    let (mut client, _snapshot) =
        prism_ipc::Client::handshake(wire, prism_ipc::Hello::new(prism_ipc::ClientKind::Desktop))
            .await
            .map_err(|error| error.to_string())?;
    client
        .send(prism_domain::Command::Shutdown)
        .await
        .map_err(|error| error.to_string())?;
    // Waited for, because `send` only queues it: a shell that exited here could
    // take the connection down before the frame left the buffer.
    let _ = tokio::time::timeout(Duration::from_secs(2), client.next_event()).await;
    client.disconnect().await;
    Ok(())
}

/// Brings the start-up entry into line with the switch, at start.
#[allow(
    clippy::print_stderr,
    reason = "the shell has no logger of its own: the daemon owns the log file"
)]
fn reconcile_at_start(app: &AppHandle, data_dir: &Path) {
    let desk: State<'_, Desk> = app.state();
    let Some(entry) = autostart::platform_entry() else {
        return;
    };
    let wanted = autostart::stored_switch(data_dir);
    if let Err(error) = autostart::apply(entry.as_ref(), wanted, &desk.command_line()) {
        // A message and never a refusal to start: a shell that would not open
        // because it could not write a registry value would be a shell that
        // cannot be used to turn the switch off again.
        eprintln!("PrismDMX: the start-up entry could not be brought into line: {error}");
    }
}

// ---- What the page may ask the shell for ----

/// **B31.** Opens the operating system's own dialogue and returns the chosen
/// path, or `None` if the operator cancelled.
///
/// Asynchronous and never blocking, because the blocking form of a native
/// dialogue must not be called on the thread the window is running on.
#[tauri::command]
async fn choose_path(
    app: AppHandle,
    kind: PathKind,
    start: Option<String>,
) -> Result<Option<String>, String> {
    let chooser = chooser(kind);
    let mut builder = app.dialog().file().set_title(chooser.title);
    for filter in chooser.filters {
        builder = builder.add_filter(filter.name, filter.extensions);
    }
    // Where the dialogue opens: beside the file the box already names, so a
    // *Save as* on an open show starts in that show's own folder rather than in
    // whatever directory the operator last used in another program.
    if let Some(start) = start.as_deref().map(Path::new) {
        if let Some(parent) = start.parent().filter(|parent| parent.is_dir()) {
            builder = builder.set_directory(parent);
        }
        if let Some(name) = start.file_name().and_then(|name| name.to_str()) {
            builder = builder.set_file_name(name);
        }
    } else if let Some(suggested) = chooser.suggested {
        builder = builder.set_file_name(suggested);
    }

    let (answer, receiver) = tokio::sync::oneshot::channel();
    match chooser.mode {
        Mode::OpenFile => builder.pick_file(move |chosen| {
            let _ = answer.send(chosen);
        }),
        Mode::SaveFile => builder.save_file(move |chosen| {
            let _ = answer.send(chosen);
        }),
        Mode::OpenDirectory => builder.pick_folder(move |chosen| {
            let _ = answer.send(chosen);
        }),
    }
    let chosen = receiver
        .await
        .map_err(|_| "the file dialogue closed without answering".to_owned())?;
    Ok(chosen.and_then(|path| {
        path.into_path()
            .ok()
            .map(|path| path.to_string_lossy().into_owned())
    }))
}

/// **B42.** Puts the desk window into full screen, or takes it out, and answers
/// with the state it actually reached.
///
/// # The window owns full screen, not the page
///
/// There are two mechanisms and they are not equivalent. A page can ask for the
/// browser's Fullscreen API, which makes the *document* fill the screen inside
/// whatever frame the host gives it; a window can be made full screen by the
/// window manager, which is what takes the title bar away. `CLAUDE.md` asks for
/// a device screen, and a title bar is the last thing on this one that is not
/// one — so in the shell it is the **window**, through Tauri's own API, and the
/// page does not touch its own Fullscreen API at all.
///
/// The browser build keeps the same two keys and uses the Fullscreen API there,
/// because that is the only thing a browser has and it is not nothing: the Web
/// Remote (S31) and the end-to-end suite both run in one. What matters is that
/// neither build gets a key that does nothing, which is what
/// `ui/src/shell/fullscreen.ts` chooses between.
///
/// Answering with the state **reached** rather than the state asked for is the
/// same rule the autostart switch follows: a control that says what it wanted
/// rather than what happened is a control that displays a lie.
#[tauri::command]
fn set_fullscreen(app: AppHandle, on: bool) -> Result<bool, String> {
    let Some(window) = app.get_webview_window(DESK) else {
        return Err("this shell has no desk window".to_owned());
    };
    window
        .set_fullscreen(on)
        .map_err(|error| error.to_string())?;
    window.is_fullscreen().map_err(|error| error.to_string())
}

/// What this machine's start-up entry actually is — the reading that stops the
/// switch displaying a lie.
#[tauri::command]
fn autostart_state(desk: State<'_, Desk>) -> Report {
    let Some(entry) = autostart::platform_entry() else {
        return Report::unsupported();
    };
    let mine = desk.command_line();
    entry.read().map_or_else(
        |_| Report::unsupported(),
        |found| autostart::report(true, found, &mine),
    )
}

/// Writes or removes the entry, the moment the box is ticked.
#[tauri::command]
fn autostart_apply(desk: State<'_, Desk>, wanted: bool) -> Result<Report, String> {
    let Some(entry) = autostart::platform_entry() else {
        return Ok(Report::unsupported());
    };
    autostart::apply(entry.as_ref(), wanted, &desk.command_line())
}

#[cfg(test)]
mod tests {
    use super::{RUNNING_TOOLTIP, STOPPED_TOOLTIP, refusal, urlencode};
    use crate::attach::Approach;
    use std::path::Path;

    /// Every way a start can end without a window, and the one way it does not.
    ///
    /// The point of the function being separate is this test: a release build
    /// has no terminal, so *the engine could not be started* has to reach a
    /// person through a dialogue or it does not reach them at all.
    #[test]
    fn a_shell_that_cannot_show_a_desk_has_a_sentence_for_it() {
        let dir = Path::new(r"C:\data");
        assert_eq!(
            refusal(
                &Ok(Approach::Attach {
                    url: "ws://127.0.0.1:7373/ipc".to_owned(),
                    token: None,
                    pid: 1,
                }),
                dir
            ),
            None
        );

        let unreachable = refusal(
            &Ok(Approach::Unreachable {
                pid: 4711,
                endpoints: "no endpoint".to_owned(),
            }),
            dir,
        )
        .expect("a desk that cannot be reached is a sentence");
        assert!(unreachable.contains("4711"), "{unreachable}");

        let failed = refusal(&Err("the engine could not be started".to_owned()), dir)
            .expect("a failure is a sentence");
        assert_eq!(failed, "the engine could not be started");

        // The arm `main` resolves before this is called. It has a sentence
        // anyway, because an arm that cannot happen and says nothing is an arm
        // that shows an empty window when it turns out it can.
        assert!(
            refusal(&Ok(Approach::Spawn), dir)
                .expect("even the unreachable arm says something")
                .contains("second one")
        );
    }

    /// **The icon reports it** — B39, and the smallest half of it.
    ///
    /// A tray icon has one line of text and it is the only thing an operator
    /// sees without clicking. The two readings have to differ and the second
    /// has to say what happened, or the icon goes on claiming a desk is running
    /// after it has stopped — which is the entry in one sentence.
    #[test]
    fn the_tray_says_which_of_the_two_states_the_desk_is_in() {
        assert_ne!(RUNNING_TOOLTIP, STOPPED_TOOLTIP);
        assert!(RUNNING_TOOLTIP.contains("running"), "{RUNNING_TOOLTIP}");
        assert!(STOPPED_TOOLTIP.contains("stopped"), "{STOPPED_TOOLTIP}");
    }

    /// The two values that go through it: a WebSocket URL and a token the
    /// daemon generated.
    #[test]
    fn a_url_survives_being_a_query_parameter() {
        assert_eq!(
            urlencode("ws://127.0.0.1:7373/ipc"),
            "ws%3A%2F%2F127.0.0.1%3A7373%2Fipc"
        );
        assert_eq!(urlencode("abcXYZ019-_.~"), "abcXYZ019-_.~");
        assert_eq!(urlencode("a&b=c"), "a%26b%3Dc");
    }
}
