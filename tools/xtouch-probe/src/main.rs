//! S20's bring-up tool: the tables of `docs/MCU_MAPPING.md` §2 held against a
//! real Behringer X-Touch.
//!
//! # Why this is a separate crate rather than a test
//!
//! `CLAUDE.md` allows no test to require hardware, and `ARCHITECTURE_SPEC.md`
//! §10.1 allows `prism-surface` no platform code. Opening a MIDI port is
//! platform code. So the port lives here, outside the workspace (see
//! `Cargo.toml`), and `prism-surface` is a path dependency: **every byte this
//! tool sends was produced by `Feedback::encode_into` and every byte it reads
//! back was decoded by `McuCodec`.** What is verified is therefore the shipping
//! codec and not a transcription of it.
//!
//! The two exceptions are deliberate and marked in the output: the `raw`
//! command, and the handful of steps that send bytes the codec **refuses** to
//! encode — a colour byte outside the eight, a strip index above seven. Asking
//! what the surface does with a message our codec will not send is exactly the
//! question `docs/MCU_MAPPING.md` §7 raises, and it can only be asked by hand.
//!
//! The permanent record is a capture file: `crates/prism-surface/tests/captures`
//! holds them and `tests/hardware_capture.rs` replays them in the ordinary
//! suite, so the desk's own bytes keep the table honest on a build server with
//! nothing plugged in — for ever, rather than only on the evening somebody had
//! the desk on the bench.
//!
//! # Commands
//!
//! ```text
//!   ports                    the MIDI ports on this machine
//!   identify                 the SysEx queries: serial, firmware, Device Ready
//!   capture [seconds]        log and analyse everything the surface sends
//!   buttons  [seconds]       the same, checked against the expected press order
//!   latency  [count]         round-trip time through the surface
//!   pacing   [count]         whether a burst of messages loses its tail
//!   raw <hex> ...            send bytes by hand
//!
//!   leds | motors | rings | meters | segments | lcd | colors | oddities
//!                            outbound experiments. With no argument they list
//!                            their steps; `<name> 3` runs step 3 alone; `<name>
//!                            all` runs the lot with PRISM_XT_STEP_MS between.
//! ```
//!
//! `PRISM_XT_IN` and `PRISM_XT_OUT` pick the ports by name substring or index;
//! `PRISM_XT_CAPTURE` is where the capture file goes.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use prism_surface::{
    ButtonId, ControlEvent, FADER_MAX, Fader, Feedback, GlobalButton, LcdMeterMode, LedState,
    MAX_MESSAGE_BYTES, METER_LEVEL_0DB, McuCodec, MeterSignal, RingMode, SegmentChar, StripButton,
    StripColor, VPotRing, X_TOUCH,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, rest) = args
        .split_first()
        .map_or(("capture", &[][..]), |(head, tail)| (head.as_str(), tail));

    if let Err(problem) = run(command, rest) {
        eprintln!("\n!! {problem}");
        std::process::exit(1);
    }
}

fn run(command: &str, args: &[String]) -> Result<(), String> {
    if command == "ports" {
        return list_ports();
    }
    let mut link = Link::open()?;
    match command {
        "capture" | "monitor" => capture(&mut link, seconds(args.first(), 60), false),
        "buttons" => capture(&mut link, seconds(args.first(), 240), true),
        "identify" => identify(&mut link),
        "pair" => pair(&mut link, args),
        "latency" => latency(&mut link, args),
        "pacing" => pacing(&mut link, args),
        "flood" => flood(&mut link, args),
        "raw" => raw(&mut link, args),
        name => match experiment(name) {
            Some(steps) => walk(&mut link, name, &steps, args),
            None => Err(format!("no such command: {name}")),
        },
    }
}

fn seconds(arg: Option<&String>, fallback: u64) -> Duration {
    Duration::from_secs(arg.and_then(|value| value.parse().ok()).unwrap_or(fallback))
}

// ---------------------------------------------------------------------------
// The port
// ---------------------------------------------------------------------------

/// A message from the surface, stamped on this machine's monotonic clock.
#[derive(Debug, Clone)]
struct Rx {
    at: Duration,
    bytes: Vec<u8>,
}

/// The two connections, the codec, and the capture file.
struct Link {
    /// Dropping this closes the input port, so it is held even though nothing
    /// reads it: the callback is the reader.
    _input: MidiInputConnection<Sender<Rx>>,
    output: MidiOutputConnection,
    inbox: Receiver<Rx>,
    codec: McuCodec,
    capture: Option<std::fs::File>,
    quiet: bool,
}

impl Link {
    fn open() -> Result<Self, String> {
        let mut input = MidiInput::new("prism-xtouch-probe").map_err(text)?;
        // The whole point is to see everything: a SysEx reply, an active
        // sensing byte, a timing clock. The codec counts what it cannot use.
        input.ignore(Ignore::None);
        let in_ports = input.ports();
        let in_names: Vec<String> = in_ports
            .iter()
            .map(|port| input.port_name(port).unwrap_or_default())
            .collect();
        let in_port = choose(&in_names, &std::env::var("PRISM_XT_IN").unwrap_or_default())
            .ok_or("no MIDI input port looks like an X-Touch - run `ports`")?;

        let output = MidiOutput::new("prism-xtouch-probe").map_err(text)?;
        let out_ports = output.ports();
        let out_names: Vec<String> = out_ports
            .iter()
            .map(|port| output.port_name(port).unwrap_or_default())
            .collect();
        let out_port = choose(&out_names, &std::env::var("PRISM_XT_OUT").unwrap_or_default())
            .ok_or("no MIDI output port looks like an X-Touch - run `ports`")?;

        println!("in   {}", in_names[in_port]);
        println!("out  {}", out_names[out_port]);

        let (sink, inbox) = mpsc::channel();
        let started = Instant::now();
        let connection = input
            .connect(
                &in_ports[in_port],
                "prism-in",
                move |_stamp, message, sink: &mut Sender<Rx>| {
                    let _ = sink.send(Rx {
                        at: started.elapsed(),
                        bytes: message.to_vec(),
                    });
                },
                sink,
            )
            .map_err(text)?;
        let output = output
            .connect(&out_ports[out_port], "prism-out")
            .map_err(text)?;

        let capture = match std::env::var("PRISM_XT_CAPTURE") {
            Ok(path) if !path.is_empty() => {
                let mut file = std::fs::File::create(&path).map_err(text)?;
                let _ = writeln!(file, "# X-Touch capture, port {}", in_names[in_port]);
                let _ = writeln!(file, "# <microseconds> <hex bytes>");
                println!("log  {path}");
                Some(file)
            }
            _ => None,
        };

        Ok(Self {
            _input: connection,
            output,
            inbox,
            codec: McuCodec::new(X_TOUCH),
            capture,
            quiet: false,
        })
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        println!("  -> {}", hex(bytes));
        self.output.send(bytes).map_err(text)
    }

    /// Sends a message the codec produced, which is the point: the bytes on the
    /// wire are `prism-surface`'s, not this tool's.
    fn show(&mut self, feedback: Feedback<'_>) -> Result<(), String> {
        let mut buf = [0u8; MAX_MESSAGE_BYTES];
        let written = feedback
            .encode_into(&X_TOUCH, &mut buf)
            .map_err(|error| format!("{feedback:?}: {error}"))?;
        let bytes = buf.get(..written).ok_or("encode wrote past the buffer")?;
        println!("  -> {}", hex(bytes));
        self.output.send(bytes).map_err(text)
    }

    /// The next message, or `None` if none arrives inside `timeout`.
    ///
    /// Separate from [`drain`](Self::drain) because a round-trip measurement has
    /// to stop at the *answer* rather than at the end of a window: the first
    /// version of `latency` drained for 500 ms and duly reported 500 ms.
    fn recv(&mut self, timeout: Duration) -> Option<Rx> {
        match self.inbox.recv_timeout(timeout) {
            Ok(rx) => {
                self.record(&rx);
                if !self.quiet {
                    self.report(&rx);
                }
                Some(rx)
            }
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => None,
        }
    }

    /// Whatever is already waiting, without blocking.
    fn poll_now(&mut self) -> Option<Rx> {
        match self.inbox.try_recv() {
            Ok(rx) => {
                self.record(&rx);
                Some(rx)
            }
            Err(_) => None,
        }
    }

    /// Everything that arrives inside `window`, printed as it comes.
    fn drain(&mut self, window: Duration) -> Vec<Rx> {
        let deadline = Instant::now() + window;
        let mut out = Vec::new();
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match self.recv(left) {
                Some(rx) => out.push(rx),
                None => break,
            }
        }
        out
    }

    /// Everything that arrives until `window` passes with `silence` of quiet.
    fn drain_until_quiet(&mut self, window: Duration, silence: Duration) -> Vec<Rx> {
        let deadline = Instant::now() + window;
        let mut out = Vec::new();
        loop {
            let left = match deadline.checked_duration_since(Instant::now()) {
                Some(left) => left.min(silence),
                None => break,
            };
            match self.recv(left) {
                Some(rx) => out.push(rx),
                None => {
                    println!("  (silence)");
                    break;
                }
            }
        }
        out
    }

    fn record(&mut self, rx: &Rx) {
        if let Some(file) = self.capture.as_mut() {
            let _ = writeln!(file, "{} {}", rx.at.as_micros(), hex(&rx.bytes));
        }
    }

    /// Prints both halves: the bytes as they arrived and what `prism-surface`
    /// made of them.
    fn report(&mut self, rx: &Rx) {
        let line = described(&mut self.codec, rx);
        println!("  <- {:>10.3} ms  {line}", millis(rx.at));
    }

    fn events(&mut self, rx: &Rx) -> Vec<ControlEvent> {
        let mut out = Vec::new();
        self.codec.push(&rx.bytes, rx.at, |event| out.push(event));
        out
    }

    fn counters(&self) {
        let counters = self.codec.counters();
        println!(
            "\n  wire: {} bytes, {} messages, {} discarded | profile: {} unmapped, {} sysex ignored",
            counters.wire.bytes,
            counters.wire.messages,
            counters.wire.discarded(),
            counters.unmapped,
            counters.sysex_ignored
        );
        if counters.wire.realtime > 0 {
            println!(
                "  {} real-time bytes (active sensing / clock)",
                counters.wire.realtime
            );
        }
    }
}

/// Decodes into a readable line, keeping the raw bytes beside the reading.
fn described(codec: &mut McuCodec, rx: &Rx) -> String {
    let mut events = String::new();
    codec.push(&rx.bytes, rx.at, |event| {
        let _ = write!(events, "{}", name_of(&event));
    });
    if events.is_empty() {
        events.push_str("(no control event)");
    }
    let raw = hex(&rx.bytes);
    if raw.len() > 40 {
        format!("{raw}\n                  {events}")
    } else {
        format!("{raw:<40}  {events}")
    }
}

fn name_of(event: &ControlEvent) -> String {
    match *event {
        ControlEvent::Button { button, pressed } => {
            format!("{button} {}", if pressed { "down" } else { "up" })
        }
        ControlEvent::Touch { fader, touched } => {
            format!("{fader} {}", if touched { "touched" } else { "released" })
        }
        ControlEvent::Move { fader, position } => format!("{fader} = {position}"),
        ControlEvent::VPot { strip, steps } => format!("Strip[{strip}].Encoder {steps:+}"),
        ControlEvent::Jog { steps } => format!("Global.Jog {steps:+}"),
    }
}

fn list_ports() -> Result<(), String> {
    let mut input = MidiInput::new("prism-xtouch-probe").map_err(text)?;
    input.ignore(Ignore::None);
    println!("MIDI inputs:");
    for (index, port) in input.ports().iter().enumerate() {
        println!("  [{index}] {}", input.port_name(port).map_err(text)?);
    }
    let output = MidiOutput::new("prism-xtouch-probe").map_err(text)?;
    println!("MIDI outputs:");
    for (index, port) in output.ports().iter().enumerate() {
        println!("  [{index}] {}", output.port_name(port).map_err(text)?);
    }
    Ok(())
}

/// Picks a port: an index, a name substring, or the first X-Touch that is not
/// the surface's second (MIDI DIN) port.
fn choose(names: &[String], want: &str) -> Option<usize> {
    if let Ok(index) = want.parse::<usize>() {
        return (index < names.len()).then_some(index);
    }
    if !want.is_empty() {
        return names.iter().position(|name| name.contains(want));
    }
    names
        .iter()
        .position(|name| name.contains("X-Touch") && !name.contains("2 (X-Touch)"))
        .or_else(|| names.iter().position(|name| name.contains("X-Touch")))
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for byte in bytes {
        let _ = write!(out, "{byte:02X} ");
    }
    out.trim_end().to_owned()
}

fn printable(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| {
            if (0x20..0x7F).contains(&byte) {
                byte as char
            } else {
                '.'
            }
        })
        .collect()
}

fn millis(at: Duration) -> f64 {
    at.as_secs_f64() * 1000.0
}

fn text<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

fn heading(title: &str) {
    println!(
        "\n=== {title} {}",
        "=".repeat(64_usize.saturating_sub(title.len()))
    );
}

// ---------------------------------------------------------------------------
// Inbound
// ---------------------------------------------------------------------------

/// The whole inbound half: log everything, then say what it was.
///
/// With `expect_walk` the press order is also checked against the order the
/// profile puts the buttons in, which is how a 104-row table gets verified
/// without anybody transcribing hex.
fn capture(link: &mut Link, window: Duration, expect_walk: bool) -> Result<(), String> {
    heading("capture");
    if expect_walk {
        println!("Expected press order:");
        for (index, (label, note)) in expected_walk()?.iter().enumerate() {
            println!("  {:>3}. note {note:>3}  {label}", index + 1);
        }
    }
    let silence = Duration::from_secs(
        std::env::var("PRISM_XT_SILENCE")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(20),
    );
    println!("\nListening for up to {window:?}, stopping after {silence:?} of silence.");
    let received = link.drain_until_quiet(window, silence);
    analyse(link, &received, expect_walk)
}

/// Every strip button, then every global button, in the order the profile puts
/// them in — which is the order the panel is walked.
///
/// `PRISM_XT_FROM` starts the list part-way in, so a 104-button walk can be done
/// in chunks without a mistake in one of them invalidating the rest.
fn expected_walk() -> Result<Vec<(String, u8)>, String> {
    let mut expected = Vec::new();
    for button in StripButton::ALL {
        for strip in 0..X_TOUCH.strips {
            let note = X_TOUCH
                .strip_note(strip, button)
                .ok_or("the profile has no note for a strip it claims")?;
            expected.push((format!("strip {} {button}", strip + 1), note));
        }
    }
    for row in X_TOUCH.buttons {
        expected.push((format!("{:?}", row.button), row.note));
    }
    let from: usize = std::env::var("PRISM_XT_FROM")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    Ok(expected.split_off(from.saturating_sub(1).min(expected.len())))
}

#[expect(clippy::too_many_lines, reason = "one report, printed once, read once")]
fn analyse(link: &mut Link, received: &[Rx], expect_walk: bool) -> Result<(), String> {
    heading("what arrived");
    let mut presses: Vec<(u8, Duration)> = Vec::new();
    let mut faders: BTreeMap<u8, (Vec<u16>, Vec<u8>)> = BTreeMap::new();
    let mut touches: Vec<u8> = Vec::new();
    let mut controllers: BTreeMap<u8, Vec<(Duration, u8)>> = BTreeMap::new();
    let mut undecoded: Vec<Rx> = Vec::new();
    let mut sysex: Vec<Rx> = Vec::new();

    for rx in received {
        let status = rx.bytes.first().copied().unwrap_or_default();
        let first = rx.bytes.get(1).copied().unwrap_or_default();
        let second = rx.bytes.get(2).copied().unwrap_or_default();
        if status == 0xF0 {
            sysex.push(rx.clone());
            continue;
        }
        let events = link.events(rx);
        if events.is_empty() {
            undecoded.push(rx.clone());
            continue;
        }
        for event in events {
            match event {
                ControlEvent::Button { pressed: true, .. } => presses.push((first, rx.at)),
                ControlEvent::Button { pressed: false, .. } => {}
                ControlEvent::Touch { touched: true, .. } => touches.push(first),
                ControlEvent::Touch { touched: false, .. } => {}
                ControlEvent::Move { position, .. } => {
                    let entry = faders.entry(status & 0x0F).or_default();
                    entry.0.push(position);
                    if entry.1.is_empty() {
                        entry.1 = rx.bytes.clone();
                    }
                }
                ControlEvent::VPot { .. } | ControlEvent::Jog { .. } => {
                    controllers.entry(first).or_default().push((rx.at, second));
                }
            }
        }
    }

    if !presses.is_empty() {
        println!("\nButton presses, in the order they arrived:");
        let expected = expected_walk()?;
        let mut mismatches = 0;
        for (index, (note, _)) in presses.iter().enumerate() {
            let label = label_of(*note);
            match expected.get(index) {
                Some((wanted_label, wanted_note)) if expect_walk => {
                    let verdict = if wanted_note == note {
                        "ok".to_owned()
                    } else {
                        mismatches += 1;
                        format!("MISMATCH - expected note {wanted_note} ({wanted_label})")
                    };
                    println!("  {:>3}. note {note:>3}  {label:<34} {verdict}", index + 1);
                }
                _ => println!("  {:>3}. note {note:>3}  {label}", index + 1),
            }
        }
        let mut seen: Vec<u8> = presses.iter().map(|(note, _)| *note).collect();
        let total = seen.len();
        seen.sort_unstable();
        seen.dedup();
        println!(
            "  {total} presses, {} distinct notes, {mismatches} in the wrong place",
            seen.len()
        );
        if expect_walk {
            let missing: Vec<String> = expected
                .iter()
                .filter(|(_, note)| !seen.contains(note))
                .map(|(label, note)| format!("{note} ({label})"))
                .collect();
            println!(
                "  never pressed ({}): {}",
                missing.len(),
                missing.join(", ")
            );
            let unexpected: Vec<u8> = seen
                .iter()
                .copied()
                .filter(|note| !expected.iter().any(|(_, want)| want == note))
                .collect();
            if !unexpected.is_empty() {
                println!("  notes not in the table at all: {unexpected:?}");
            }
        }
    }

    if !touches.is_empty() {
        let mut order = Vec::new();
        for note in &touches {
            if !order.contains(note) {
                order.push(*note);
            }
        }
        println!("\nFader touch notes, in first-touch order: {order:?}");
        for note in &order {
            println!("  note {note:>3} -> {}", label_of(*note));
        }
    }

    if !faders.is_empty() {
        println!("\nFaders (pitch bend), by channel:");
        for (channel, (positions, first)) in &faders {
            let low = positions.iter().min().copied().unwrap_or_default();
            let high = positions.iter().max().copied().unwrap_or_default();
            println!(
                "  channel {:>2} (status {:#04X}) -> {:<14} {} messages, {low}..{high} of {FADER_MAX}",
                channel + 1,
                0xE0 | channel,
                X_TOUCH
                    .fader_on_channel(*channel)
                    .map_or_else(|| "(not in table)".to_owned(), |fader| fader.to_string()),
                positions.len()
            );
            println!("     first message {} - LSB first puts the small change in byte 2", hex(first));
        }
    }

    if !controllers.is_empty() {
        println!("\nRelative controls (control change), by controller:");
        for (controller, samples) in &controllers {
            let mut values: Vec<u8> = samples.iter().map(|(_, value)| *value).collect();
            values.sort_unstable();
            values.dedup();
            let gaps: Vec<f64> = samples
                .windows(2)
                .map(|pair| millis(pair[1].0.saturating_sub(pair[0].0)))
                .collect();
            let smallest = gaps.iter().copied().fold(f64::INFINITY, f64::min);
            let mean = if gaps.is_empty() {
                0.0
            } else {
                gaps.iter().sum::<f64>() / gaps.len() as f64
            };
            let magnitude = values.iter().map(|value| value & 0x3F).max().unwrap_or(0);
            let which = if *controller == X_TOUCH.jog_cc {
                "Global.Jog".to_owned()
            } else {
                X_TOUCH
                    .vpot_strip_at(*controller)
                    .map_or_else(|| "(not in table)".to_owned(), |strip| {
                        format!("Strip[{strip}].Encoder")
                    })
            };
            println!(
                "  CC {controller:>3} -> {which:<22} {} messages, data bytes {:02X?}",
                samples.len(),
                values
            );
            println!(
                "     largest magnitude {magnitude}, gap smallest {smallest:.2} ms, mean {mean:.2} ms"
            );
        }
    }

    if !sysex.is_empty() {
        println!("\nSystem exclusive:");
        for rx in &sysex {
            println!("  {} | {}", hex(&rx.bytes), printable(&rx.bytes));
        }
    }

    if !undecoded.is_empty() {
        println!("\nMessages the profile does not describe:");
        for rx in &undecoded {
            println!("  {}", hex(&rx.bytes));
        }
    }

    link.counters();
    Ok(())
}

/// What the profile calls a note number, whichever of its two tables it is in.
fn label_of(note: u8) -> String {
    if let Some(fader) = X_TOUCH.touch_at(note) {
        return format!("{fader} touch");
    }
    if let Some((strip, button)) = X_TOUCH.strip_button_at(note) {
        return format!("Strip[{strip}].Button.{button}");
    }
    X_TOUCH
        .button_at(note)
        .map_or_else(|| "(not in the table)".to_owned(), |button| {
            format!("Global.{button:?}")
        })
}

/// The note map checked in both directions at once, and without an order to
/// keep to: light one LED, and see which note the button under it sends.
///
/// This is the protocol that makes a 64-row table checkable by a person. A press
/// order printed on a screen has to be followed exactly or the comparison is
/// meaningless — and the first attempt at that produced a walk in a different
/// order and 71 "mismatches" that were nothing of the kind. Here the surface
/// itself says which button to press, so an error is a real disagreement between
/// the outbound note and the inbound one, and **both halves of the table are
/// verified in one pass**: if the LED map were wrong the wrong button would
/// light, and the note that came back would say so.
fn pair(link: &mut Link, args: &[String]) -> Result<(), String> {
    heading("pair");
    let from: usize = args
        .first()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    let patience = Duration::from_secs(
        std::env::var("PRISM_XT_PATIENCE")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(30),
    );
    link.quiet = true;
    let mut matched = 0_usize;
    let mut wrong: Vec<(String, u8, u8)> = Vec::new();
    let mut dark: Vec<String> = Vec::new();

    let count: usize = args
        .get(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(usize::MAX);
    for (index, row) in X_TOUCH
        .buttons
        .iter()
        .enumerate()
        .skip(from.saturating_sub(1))
        .take(count)
    {
        let label = format!("{:?}", row.button);
        link.show(Feedback::Led {
            button: ButtonId::Global(row.button),
            state: LedState::On,
        })?;
        print!("  {:>2}/64  lit note {:>3} ({label:<22}) ... ", index + 1, row.note);
        let _ = std::io::stdout().flush();
        let deadline = Instant::now() + patience;
        let mut answer = None;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let Some(rx) = link.recv(left) else { break };
            let note = rx.bytes.get(1).copied().unwrap_or_default();
            if link
                .events(&rx)
                .iter()
                .any(|event| matches!(event, ControlEvent::Button { pressed: true, .. }))
            {
                answer = Some(note);
                break;
            }
        }
        link.show(Feedback::Led {
            button: ButtonId::Global(row.button),
            state: LedState::Off,
        })?;
        match answer {
            Some(note) if note == row.note => {
                println!("OK");
                matched += 1;
            }
            Some(note) => {
                println!("MISMATCH: the button that lit sent note {note} ({})", label_of(note));
                wrong.push((label, row.note, note));
            }
            None => {
                println!("nothing pressed");
                dark.push(label);
            }
        }
    }

    heading("pair result");
    println!("  {matched} of 64 agree in both directions");
    for (label, lit, sent) in &wrong {
        println!("  {label}: LED on note {lit}, press sent {sent}");
    }
    println!("  {} not answered: {}", dark.len(), dark.join(", "));
    link.counters();
    Ok(())
}

/// What the surface answers to the queries in `docs/MCU_MAPPING.md` §2.3, and to
/// the one command that turns out to name a firmware.
fn identify(link: &mut Link) -> Result<(), String> {
    heading("identify");
    link.drain(Duration::from_millis(300));
    for (label, command) in [
        ("device query (0x00)", 0x00_u8),
        ("version request (0x13)", 0x13),
    ] {
        println!("\n{label}");
        link.send(&[0xF0, 0x00, 0x00, 0x66, X_TOUCH.device_id, command, 0xF7])?;
        let replies = link.drain(Duration::from_millis(900));
        if replies.is_empty() {
            println!("  (no answer)");
        }
        for reply in &replies {
            println!("     as text: {}", printable(&reply.bytes));
        }
    }
    link.counters();
    Ok(())
}

/// Round-trip time through the surface, measured the only way a device with no
/// read-back allows: the optional handshake.
fn latency(link: &mut Link, args: &[String]) -> Result<(), String> {
    heading("latency");
    let count: usize = args
        .first()
        .and_then(|value| value.parse().ok())
        .unwrap_or(50);
    link.quiet = true;
    link.drain(Duration::from_millis(200));
    let query = [0xF0, 0x00, 0x00, 0x66, X_TOUCH.device_id, 0x00, 0xF7];
    let mut samples = Vec::new();
    for _ in 0..count {
        let sent = Instant::now();
        link.output.send(&query).map_err(text)?;
        if link.recv(Duration::from_millis(500)).is_none() {
            continue;
        }
        samples.push(sent.elapsed());
        std::thread::sleep(Duration::from_millis(20));
    }
    if samples.is_empty() {
        println!("  the surface answered nothing; no round trip to measure");
        return Ok(());
    }
    samples.sort_unstable();
    let mean = samples.iter().sum::<Duration>() / u32::try_from(samples.len()).unwrap_or(1);
    println!(
        "  {} of {count} round trips: min {:.3} ms, median {:.3} ms, max {:.3} ms, mean {:.3} ms",
        samples.len(),
        millis(samples[0]),
        millis(samples[samples.len() / 2]),
        millis(samples[samples.len() - 1]),
        millis(mean)
    );
    println!("  Host -> surface -> host, so one direction is about half of it.");
    Ok(())
}

/// Does a burst lose its tail, and at what spacing does it stop?
///
/// Automatic, and it needs no eyes: the surface answers a device query, so *n*
/// queries fired back to back should produce *n* answers and the ones that never
/// come back are the tail that was lost. Two independent projects report the
/// X-Touch dropping messages under exactly this treatment.
fn pacing(link: &mut Link, args: &[String]) -> Result<(), String> {
    heading("pacing");
    let count: usize = args
        .first()
        .and_then(|value| value.parse().ok())
        .unwrap_or(64);
    let query = [0xF0, 0x00, 0x00, 0x66, X_TOUCH.device_id, 0x00, 0xF7];
    link.quiet = true;
    link.drain(Duration::from_millis(300));
    link.output.send(&query).map_err(text)?;
    if link.drain(Duration::from_millis(900)).is_empty() {
        return Err("no answer to a device query, so there is nothing to count".to_owned());
    }

    println!("  {count} device queries per round.\n");
    for gap_us in [0_u64, 200, 500, 1000, 2000, 5000] {
        link.drain(Duration::from_millis(400));
        let started = Instant::now();
        for _ in 0..count {
            link.output.send(&query).map_err(text)?;
            if gap_us > 0 {
                std::thread::sleep(Duration::from_micros(gap_us));
            }
        }
        let sending = started.elapsed();
        let replies = link.drain(Duration::from_secs(3)).len();
        println!(
            "  gap {gap_us:>5} us ({:>6.1} ms to send): {replies:>3} answers, {} missing",
            millis(sending),
            count.saturating_sub(replies)
        );
    }

    println!("\n  The same again with the biggest message the codec sends, a full");
    println!("  112-character scribble strip write (63 bytes on the wire).");
    let mut buf = [0u8; MAX_MESSAGE_BYTES];
    let filler = [b'*'; 112];
    let written = Feedback::DisplayText {
        offset: 0,
        text: &filler,
    }
    .encode_into(&X_TOUCH, &mut buf)
    .map_err(text)?;
    let long = buf.get(..written).ok_or("encode wrote past the buffer")?.to_vec();
    for gap_us in [0_u64, 1000, 5000] {
        link.drain(Duration::from_millis(400));
        for index in 0..count {
            // Interleave the queries with the long messages, then count the
            // answers: a query that never comes back was dropped along with
            // whatever was around it.
            link.output.send(&long).map_err(text)?;
            if gap_us > 0 {
                std::thread::sleep(Duration::from_micros(gap_us));
            }
            link.output.send(&query).map_err(text)?;
            if gap_us > 0 {
                std::thread::sleep(Duration::from_micros(gap_us));
            }
            let _ = index;
        }
        let replies = link.drain(Duration::from_secs(3)).len();
        println!(
            "  gap {gap_us:>5} us, {count} long + {count} queries: {replies:>3} answers, {} missing",
            count.saturating_sub(replies)
        );
    }
    link.counters();
    Ok(())
}

/// The pacing question, asked carefully, because getting it wrong costs a power
/// cycle.
///
/// `flood <gap_us> <count> [silent|interleave]`. A round sends `count` full
/// 112-character scribble strip writes — the largest message this codec emits,
/// 63 bytes on the wire — with `gap_us` between them. In `interleave` mode a
/// device query follows each one and its answer is waited for, which is how the
/// exact message at which the surface stops talking is found. In `silent` mode
/// the burst is sent with nothing asked of it and one query goes out at the end,
/// which separates *the flood* from *the asking*.
///
/// It stops after three consecutive unanswered queries: once the surface has
/// stopped answering there is nothing further to learn from the round, and every
/// extra message is a message sent at a device that is already in the state
/// being reported.
fn flood(link: &mut Link, args: &[String]) -> Result<(), String> {
    heading("flood");
    let gap = Duration::from_micros(
        args.first()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0),
    );
    let count: usize = args
        .get(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(64);
    let mode = args.get(2).map_or("interleave", String::as_str);
    let interleave = mode == "interleave";
    if mode == "moving" {
        return flood_while_moving(link, gap, count);
    }
    if mode == "burst" {
        return flood_burst(link, gap, count);
    }
    let query = [0xF0, 0x00, 0x00, 0x66, X_TOUCH.device_id, 0x00, 0xF7];
    link.quiet = true;

    link.drain(Duration::from_millis(300));
    link.output.send(&query).map_err(text)?;
    if link.recv(Duration::from_millis(800)).is_none() {
        return Err("the surface is not answering before the test even starts".to_owned());
    }
    println!(
        "  alive. {count} x 63-byte SysEx, {gap:?} apart, {}\n",
        if interleave {
            "each followed by a query"
        } else {
            "nothing asked until the end"
        }
    );

    let mut buf = [0u8; MAX_MESSAGE_BYTES];
    let mut answered = 0_usize;
    let mut misses = 0_usize;
    let mut first_miss = None;
    let started = Instant::now();
    for index in 0..count {
        let filler = [b'A' + u8::try_from(index % 26).unwrap_or(0); 112];
        let written = Feedback::DisplayText {
            offset: 0,
            text: &filler,
        }
        .encode_into(&X_TOUCH, &mut buf)
        .map_err(text)?;
        let message = buf.get(..written).ok_or("encode wrote past the buffer")?.to_vec();
        link.output.send(&message).map_err(text)?;
        if gap > Duration::ZERO {
            std::thread::sleep(gap);
        }
        if !interleave {
            continue;
        }
        link.output.send(&query).map_err(text)?;
        if link.recv(Duration::from_millis(300)).is_some() {
            answered += 1;
            misses = 0;
        } else {
            misses += 1;
            if first_miss.is_none() {
                first_miss = Some(index + 1);
            }
            if misses == 3 {
                println!(
                    "  three unanswered in a row at message {} - stopping the round",
                    index + 1
                );
                break;
            }
        }
        if gap > Duration::ZERO {
            std::thread::sleep(gap);
        }
    }
    let elapsed = started.elapsed();

    if interleave {
        println!(
            "  {answered} queries answered; first unanswered at message {}",
            first_miss.map_or_else(|| "never".to_owned(), |at| at.to_string())
        );
    }
    println!("  the burst took {:.1} ms", millis(elapsed));

    // Give it a moment: a surface that is merely busy catches up, and one that
    // has stopped talking does not.
    std::thread::sleep(Duration::from_secs(1));
    link.drain(Duration::from_millis(500));
    link.output.send(&query).map_err(text)?;
    let alive = link.recv(Duration::from_millis(1500)).is_some();
    println!(
        "\n  after the burst the surface is {}",
        if alive {
            "still answering"
        } else {
            "NOT answering - its MIDI transmitter has stopped"
        }
    );
    Ok(())
}

/// The case PrismDMX will actually be in: the desk is transmitting a fader while
/// the host writes scribble strips at it.
///
/// Nothing is asked of the surface — the operator's hand is what makes it talk —
/// so this measures whether a busy inbound stream survives a busy outbound one.
fn flood_while_moving(link: &mut Link, gap: Duration, count: usize) -> Result<(), String> {
    println!(
        "  {count} x 63-byte SysEx, {gap:?} apart, while a fader is being moved.\n\
         Keep sweeping a fader for the whole run."
    );
    // Wait until the operator is actually moving something, or there is nothing
    // to measure.
    if link.recv(Duration::from_secs(20)).is_none() {
        return Err("no inbound traffic - nothing was being moved".to_owned());
    }
    let mut inbound = 0_usize;
    let mut last = Instant::now();
    let mut buf = [0u8; MAX_MESSAGE_BYTES];
    let started = Instant::now();
    for index in 0..count {
        let filler = [b'A' + u8::try_from(index % 26).unwrap_or(0); 112];
        let written = Feedback::DisplayText {
            offset: 0,
            text: &filler,
        }
        .encode_into(&X_TOUCH, &mut buf)
        .map_err(text)?;
        let message = buf.get(..written).ok_or("encode wrote past the buffer")?.to_vec();
        link.output.send(&message).map_err(text)?;
        if gap > Duration::ZERO {
            std::thread::sleep(gap);
        }
        while link.poll_now().is_some() {
            inbound += 1;
            last = Instant::now();
        }
    }
    let elapsed = started.elapsed();
    println!(
        "  {count} messages in {:.1} ms; {inbound} messages came back during it",
        millis(elapsed)
    );
    println!(
        "  the last inbound message was {:.1} ms before the end",
        millis(elapsed.saturating_sub(last.duration_since(started)))
    );
    println!("\n  keep moving the fader - checking whether it still talks");
    let after = link.drain(Duration::from_secs(3)).len();
    println!(
        "  {after} messages in the three seconds after the burst: {}",
        if after > 0 {
            "still transmitting"
        } else {
            "NOTHING - its MIDI transmitter has stopped"
        }
    );
    Ok(())
}

/// The case that killed the surface the first time: both directions saturated at
/// once, with nothing waited for.
fn flood_burst(link: &mut Link, gap: Duration, count: usize) -> Result<(), String> {
    let query = [0xF0, 0x00, 0x00, 0x66, X_TOUCH.device_id, 0x00, 0xF7];
    link.drain(Duration::from_millis(300));
    link.output.send(&query).map_err(text)?;
    if link.recv(Duration::from_millis(800)).is_none() {
        return Err("the surface is not answering before the test even starts".to_owned());
    }
    println!(
        "  alive. {count} x (63-byte SysEx + query), {gap:?} apart, nothing waited for."
    );
    let mut buf = [0u8; MAX_MESSAGE_BYTES];
    let started = Instant::now();
    for index in 0..count {
        let filler = [b'A' + u8::try_from(index % 26).unwrap_or(0); 112];
        let written = Feedback::DisplayText {
            offset: 0,
            text: &filler,
        }
        .encode_into(&X_TOUCH, &mut buf)
        .map_err(text)?;
        let message = buf.get(..written).ok_or("encode wrote past the buffer")?.to_vec();
        link.output.send(&message).map_err(text)?;
        if gap > Duration::ZERO {
            std::thread::sleep(gap);
        }
        link.output.send(&query).map_err(text)?;
        if gap > Duration::ZERO {
            std::thread::sleep(gap);
        }
    }
    let elapsed = started.elapsed();
    let replies = link.drain(Duration::from_secs(3)).len();
    println!(
        "  sent in {:.1} ms; {replies} of {count} queries answered",
        millis(elapsed)
    );
    std::thread::sleep(Duration::from_secs(1));
    link.drain(Duration::from_millis(500));
    link.output.send(&query).map_err(text)?;
    let alive = link.recv(Duration::from_millis(1500)).is_some();
    println!(
        "\n  after the burst the surface is {}",
        if alive {
            "still answering"
        } else {
            "NOT answering - its MIDI transmitter has stopped"
        }
    );
    Ok(())
}

fn raw(link: &mut Link, args: &[String]) -> Result<(), String> {
    heading("raw");
    let bytes: Vec<u8> = args
        .iter()
        .map(|arg| {
            u8::from_str_radix(arg.trim_start_matches("0x"), 16)
                .map_err(|_| format!("{arg} is not a hex byte"))
        })
        .collect::<Result<_, _>>()?;
    if bytes.is_empty() {
        return Err("nothing to send: raw F0 00 00 66 14 00 F7".to_owned());
    }
    link.send(&bytes)?;
    link.drain(Duration::from_secs(2));
    link.counters();
    Ok(())
}

// ---------------------------------------------------------------------------
// Outbound: experiments made of steps
// ---------------------------------------------------------------------------

/// One thing to do to the surface, and what to watch while it happens.
struct Step {
    watch: &'static str,
    action: fn(&mut Link) -> Result<(), String>,
}

/// Runs one step, all of them, or lists them.
fn walk(link: &mut Link, name: &str, steps: &[Step], args: &[String]) -> Result<(), String> {
    let gap = Duration::from_millis(
        std::env::var("PRISM_XT_STEP_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(6000),
    );
    match args.first().map(String::as_str) {
        None | Some("list") => {
            heading(name);
            for (index, step) in steps.iter().enumerate() {
                println!("  {:>2}. {}", index + 1, step.watch);
            }
            println!("\n  `{name} <n>` runs one, `{name} all` runs the lot.");
            Ok(())
        }
        Some("all") => {
            for (index, step) in steps.iter().enumerate() {
                heading(&format!("{name} {}/{}", index + 1, steps.len()));
                println!("WATCH: {}", step.watch);
                (step.action)(link)?;
                link.drain(gap);
            }
            link.counters();
            Ok(())
        }
        Some(which) => {
            let index: usize = which
                .parse()
                .map_err(|_| format!("{which} is not a step number"))?;
            let step = steps
                .get(index.wrapping_sub(1))
                .ok_or_else(|| format!("{name} has {} steps", steps.len()))?;
            heading(&format!("{name} {index}/{}", steps.len()));
            println!("WATCH: {}", step.watch);
            (step.action)(link)?;
            link.drain(Duration::from_millis(1200));
            link.counters();
            Ok(())
        }
    }
}

fn experiment(name: &str) -> Option<Vec<Step>> {
    match name {
        "leds" => Some(led_steps()),
        "motors" => Some(motor_steps()),
        "rings" => Some(ring_steps()),
        "meters" => Some(meter_steps()),
        "segments" => Some(segment_steps()),
        "lcd" => Some(lcd_steps()),
        "colors" => Some(color_steps()),
        "oddities" => Some(oddity_steps()),
        _ => None,
    }
}

/// Lights one group of LEDs, which is how the outbound half of the note map gets
/// checked by eye without 104 questions.
fn group(link: &mut Link, buttons: &[GlobalButton], state: LedState) -> Result<(), String> {
    for button in buttons {
        link.show(Feedback::Led {
            button: ButtonId::Global(*button),
            state,
        })?;
        std::thread::sleep(Duration::from_millis(2));
    }
    Ok(())
}

fn dark(link: &mut Link) -> Result<(), String> {
    for note in 0..=127_u8 {
        link.output.send(&[0x90, note, 0x00]).map_err(text)?;
        std::thread::sleep(Duration::from_millis(1));
    }
    println!("  -> every note 0..127 set to velocity 0");
    Ok(())
}

fn led_steps() -> Vec<Step> {
    use GlobalButton as G;
    vec![
        Step {
            watch: "everything dark first",
            action: dark,
        },
        Step {
            watch: "Encoder Assign, 6 lit: Track Send Pan Plug-in EQ Instrument",
            action: |link| {
                group(
                    link,
                    &[
                        G::AssignTrack,
                        G::AssignSend,
                        G::AssignPan,
                        G::AssignPlugin,
                        G::AssignEq,
                        G::AssignInstrument,
                    ],
                    LedState::On,
                )
            },
        },
        Step {
            watch: "and now the fader bank row too: Bank< Bank> Chan< Chan> Flip GlobalView",
            action: |link| {
                group(
                    link,
                    &[
                        G::BankLeft,
                        G::BankRight,
                        G::ChannelLeft,
                        G::ChannelRight,
                        G::Flip,
                        G::GlobalView,
                    ],
                    LedState::On,
                )
            },
        },
        Step {
            watch: "and the two display buttons: Name/Value, SMPTE/Beats",
            action: |link| group(link, &[G::NameValue, G::SmpteBeats], LedState::On),
        },
        Step {
            watch: "and F1..F8",
            action: |link| {
                group(
                    link,
                    &[G::F1, G::F2, G::F3, G::F4, G::F5, G::F6, G::F7, G::F8],
                    LedState::On,
                )
            },
        },
        Step {
            watch: "and the Global View group of 8: MIDI Inputs AudioTracks Instruments Aux Busses Outputs User",
            action: |link| {
                group(
                    link,
                    &[
                        G::ViewMidiTracks,
                        G::ViewInputs,
                        G::ViewAudioTracks,
                        G::ViewAudioInstruments,
                        G::ViewAux,
                        G::ViewBusses,
                        G::ViewOutputs,
                        G::ViewUser,
                    ],
                    LedState::On,
                )
            },
        },
        Step {
            watch: "and the four modifiers: Shift Option Control Alt",
            action: |link| {
                group(
                    link,
                    &[G::ModShift, G::ModOption, G::ModControl, G::ModAlt],
                    LedState::On,
                )
            },
        },
        Step {
            watch: "and the six automation buttons: Read Write Trim Touch Latch Group",
            action: |link| {
                group(
                    link,
                    &[
                        G::AutoRead,
                        G::AutoWrite,
                        G::AutoTrim,
                        G::AutoTouch,
                        G::AutoLatch,
                        G::AutoGroup,
                    ],
                    LedState::On,
                )
            },
        },
        Step {
            watch: "and the four utility buttons: Save Undo Cancel Enter",
            action: |link| group(link, &[G::Save, G::Undo, G::Cancel, G::Enter], LedState::On),
        },
        Step {
            watch: "and the upper transport row of 7: Marker Nudge Cycle Drop Replace Click Solo",
            action: |link| {
                group(
                    link,
                    &[
                        G::Markers,
                        G::Nudge,
                        G::Cycle,
                        G::Drop,
                        G::Replace,
                        G::Click,
                        G::SoloClear,
                    ],
                    LedState::On,
                )
            },
        },
        Step {
            watch: "and the transport of 5: Rewind Forward Stop Play Record",
            action: |link| {
                group(
                    link,
                    &[G::Rewind, G::FastForward, G::Stop, G::Play, G::Record],
                    LedState::On,
                )
            },
        },
        Step {
            watch: "and the cursor cluster of 6: Up Down Left Right Zoom Scrub",
            action: |link| {
                group(
                    link,
                    &[
                        G::CursorUp,
                        G::CursorDown,
                        G::CursorLeft,
                        G::CursorRight,
                        G::Zoom,
                        G::Scrub,
                    ],
                    LedState::On,
                )
            },
        },
        Step {
            watch: "the two foot switch notes (102, 103) - no LED is expected to exist",
            action: |link| group(link, &[G::FootSwitch1, G::FootSwitch2], LedState::On),
        },
        Step {
            watch: "the strip buttons: Rec on all eight",
            action: |link| strip_row(link, StripButton::Rec, LedState::On),
        },
        Step {
            watch: "and Solo on all eight",
            action: |link| strip_row(link, StripButton::Solo, LedState::On),
        },
        Step {
            watch: "and Mute on all eight",
            action: |link| strip_row(link, StripButton::Mute, LedState::On),
        },
        Step {
            watch: "and Select on all eight",
            action: |link| strip_row(link, StripButton::Select, LedState::On),
        },
        Step {
            watch: "velocity 1 on Play: does the surface flash it by itself?",
            action: |link| {
                link.show(Feedback::Led {
                    button: ButtonId::Global(G::Play),
                    state: LedState::Flashing,
                })
            },
        },
        Step {
            watch: "everything dark again",
            action: dark,
        },
    ]
}

fn strip_row(link: &mut Link, button: StripButton, state: LedState) -> Result<(), String> {
    for strip in 0..X_TOUCH.strips {
        link.show(Feedback::Led {
            button: ButtonId::Strip { strip, button },
            state,
        })?;
        std::thread::sleep(Duration::from_millis(2));
    }
    Ok(())
}

fn all_faders(link: &mut Link, position: u16) -> Result<(), String> {
    for fader in (0..X_TOUCH.strips).map(Fader::Strip).chain([Fader::Main]) {
        link.show(Feedback::Move { fader, position })?;
        std::thread::sleep(Duration::from_millis(3));
    }
    Ok(())
}

fn motor_steps() -> Vec<Step> {
    vec![
        Step {
            watch: "all nine faders to the bottom (0)",
            action: |link| all_faders(link, 0),
        },
        Step {
            watch: "all nine to the top (16383)",
            action: |link| all_faders(link, FADER_MAX),
        },
        Step {
            watch: "all nine to 12700 - two implementations put the printed 0 dB mark here",
            action: |link| all_faders(link, 12700),
        },
        Step {
            watch: "all nine to 8192, half of the numeric range",
            action: |link| all_faders(link, 8192),
        },
        Step {
            watch: "a staircase: strip 1 at the bottom, strip 8 near the top, main fader at the top",
            action: |link| {
                for (index, fader) in (0..X_TOUCH.strips)
                    .map(Fader::Strip)
                    .chain([Fader::Main])
                    .enumerate()
                {
                    let position = u16::try_from(index).unwrap_or(0) * (FADER_MAX / 8);
                    link.show(Feedback::Move { fader, position })?;
                    std::thread::sleep(Duration::from_millis(3));
                }
                Ok(())
            },
        },
        Step {
            watch: "pitch bend on channel 10 - a fader this surface has not got",
            action: |link| link.send(&[0xE9, 0x00, 0x40]),
        },
        Step {
            watch: "all nine back to the bottom",
            action: |link| all_faders(link, 0),
        },
    ]
}

fn ring(link: &mut Link, mode: RingMode, position: u8, led: bool) -> Result<(), String> {
    for strip in 0..X_TOUCH.strips {
        link.show(Feedback::Ring {
            strip,
            ring: VPotRing {
                mode,
                position,
                led,
            },
        })?;
        std::thread::sleep(Duration::from_millis(2));
    }
    Ok(())
}

fn ring_steps() -> Vec<Step> {
    vec![
        Step {
            watch: "rings off",
            action: |link| ring(link, RingMode::Dot, 0, false),
        },
        Step {
            watch: "Dot mode, a rising position per strip: 1 2 3 4 5 6 7 8",
            action: |link| {
                for strip in 0..X_TOUCH.strips {
                    link.show(Feedback::Ring {
                        strip,
                        ring: VPotRing {
                            mode: RingMode::Dot,
                            position: strip + 1,
                            led: false,
                        },
                    })?;
                    std::thread::sleep(Duration::from_millis(2));
                }
                Ok(())
            },
        },
        Step {
            watch: "Dot mode, position 11 on every ring - the last LED only",
            action: |link| ring(link, RingMode::Dot, 11, false),
        },
        Step {
            watch: "BoostCut mode, position 6 - centred",
            action: |link| ring(link, RingMode::BoostCut, 6, false),
        },
        Step {
            watch: "BoostCut mode, position 11 - filled to the right of centre",
            action: |link| ring(link, RingMode::BoostCut, 11, false),
        },
        Step {
            watch: "Wrap mode, position 6 - filled from the left",
            action: |link| ring(link, RingMode::Wrap, 6, false),
        },
        Step {
            watch: "Spread mode, position 6 - symmetrical about the centre (max is 6, not 11)",
            action: |link| ring(link, RingMode::Spread, 6, false),
        },
        Step {
            watch: "the small LED under each encoder and nothing else",
            action: |link| ring(link, RingMode::Dot, 0, true),
        },
        Step {
            watch: "rings off again",
            action: |link| ring(link, RingMode::Dot, 0, false),
        },
    ]
}

fn meter_steps() -> Vec<Step> {
    vec![
        Step {
            watch: "a staircase: strip n at level n, so strip 1 is silent and strip 8 near 0 dB",
            action: |link| {
                for strip in 0..X_TOUCH.strips {
                    link.show(Feedback::Meter {
                        strip,
                        signal: MeterSignal::Level(strip),
                    })?;
                    std::thread::sleep(Duration::from_millis(2));
                }
                Ok(())
            },
        },
        Step {
            watch: "every meter held at 0 dB for three seconds, then nothing - time the fall",
            action: |link| {
                let deadline = Instant::now() + Duration::from_secs(3);
                while Instant::now() < deadline {
                    for strip in 0..X_TOUCH.strips {
                        link.output
                            .send(&[0xD0, (strip << 4) | METER_LEVEL_0DB])
                            .map_err(text)?;
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    std::thread::sleep(Duration::from_millis(60));
                }
                println!("  -> stopped sending. How long until the meters are empty?");
                Ok(())
            },
        },
        Step {
            watch: "level 13 (0x0D, above 0 dB) on every strip",
            action: |link| {
                for strip in 0..X_TOUCH.strips {
                    link.show(Feedback::Meter {
                        strip,
                        signal: MeterSignal::Level(13),
                    })?;
                    std::thread::sleep(Duration::from_millis(2));
                }
                Ok(())
            },
        },
        Step {
            watch: "the overload flag set on strips 1 and 8",
            action: |link| {
                for strip in [0_u8, 7] {
                    link.show(Feedback::Meter {
                        strip,
                        signal: MeterSignal::Overload(true),
                    })?;
                }
                Ok(())
            },
        },
        Step {
            watch: "the overload flag cleared on strips 1 and 8",
            action: |link| {
                for strip in [0_u8, 7] {
                    link.show(Feedback::Meter {
                        strip,
                        signal: MeterSignal::Overload(false),
                    })?;
                }
                Ok(())
            },
        },
        Step {
            watch: "vertical LCD meter mode",
            action: |link| link.show(Feedback::MeterMode(LcdMeterMode::Vertical)),
        },
        Step {
            watch: "back to horizontal LCD meter mode",
            action: |link| link.show(Feedback::MeterMode(LcdMeterMode::Horizontal)),
        },
    ]
}

fn segment_steps() -> Vec<Step> {
    vec![
        Step {
            watch: "digits 0..11 get 0 1 2 3 4 5 6 7 8 9 A B, rightmost digit first",
            action: |link| {
                for (digit, glyph) in b"0123456789AB".iter().enumerate() {
                    let character =
                        SegmentChar::from_ascii(*glyph).ok_or("a glyph the display cannot show")?;
                    link.show(Feedback::Segment {
                        digit: u8::try_from(digit).unwrap_or(0),
                        character,
                    })?;
                    std::thread::sleep(Duration::from_millis(2));
                }
                println!("  -> read the display left to right and write it down");
                Ok(())
            },
        },
        Step {
            watch: "the dot on the rightmost digit only",
            action: |link| {
                link.show(Feedback::Segment {
                    digit: 0,
                    character: SegmentChar::from_ascii(b'0')
                        .ok_or("zero is a character")?
                        .with_dot(),
                })
            },
        },
        Step {
            watch: "value 0 on all twelve digits: blank, or twelve @ signs?",
            action: |link| {
                for digit in 0..X_TOUCH.segments {
                    let controller = X_TOUCH
                        .segment_controller(digit)
                        .ok_or("a digit the profile claims")?;
                    link.output
                        .send(&[0xB0, controller, 0x00])
                        .map_err(text)?;
                    std::thread::sleep(Duration::from_millis(2));
                }
                println!("  -> CC 64..75 = 0");
                Ok(())
            },
        },
        Step {
            watch: "value 32 on all twelve digits - a space under the bit-6-stripped rule",
            action: |link| {
                for digit in 0..X_TOUCH.segments {
                    let controller = X_TOUCH
                        .segment_controller(digit)
                        .ok_or("a digit the profile claims")?;
                    link.output.send(&[0xB0, controller, 32]).map_err(text)?;
                    std::thread::sleep(Duration::from_millis(2));
                }
                Ok(())
            },
        },
        Step {
            watch: "P R I S M on the five rightmost timecode digits, spaces to their left",
            action: |link| {
                let word = b"     PRISM";
                for (index, glyph) in word.iter().enumerate() {
                    // Digit 0 is the rightmost, so the word is written backwards.
                    let digit = u8::try_from(word.len() - 1 - index).unwrap_or(0);
                    let character =
                        SegmentChar::from_ascii(*glyph).ok_or("a glyph the display can show")?;
                    link.show(Feedback::Segment { digit, character })?;
                    std::thread::sleep(Duration::from_millis(2));
                }
                Ok(())
            },
        },
        Step {
            watch: "88 on the two assignment digits (10 and 11)",
            action: |link| {
                for digit in [10_u8, 11] {
                    link.show(Feedback::Segment {
                        digit,
                        character: SegmentChar::from_ascii(b'8').ok_or("eight is a character")?,
                    })?;
                }
                Ok(())
            },
        },
        Step {
            watch: "the same twelve characters on MIDI channel 16 - does the display move at all?",
            action: |link| {
                for (digit, glyph) in b"AAAAAAAAAAAA".iter().enumerate() {
                    let character = SegmentChar::from_ascii(*glyph).ok_or("A is a character")?;
                    let controller = X_TOUCH
                        .segment_controller(u8::try_from(digit).unwrap_or(0))
                        .ok_or("a digit the profile claims")?;
                    link.output
                        .send(&[0xBF, controller, character.value().map_err(text)?])
                        .map_err(text)?;
                    std::thread::sleep(Duration::from_millis(2));
                }
                println!("  -> twelve A's on channel 16 (status BF)");
                Ok(())
            },
        },
        Step {
            watch: "CC 76 on channel 1 - one digit past the twelfth, sent by hand",
            action: |link| link.send(&[0xB0, 76, 0x0F]),
        },
    ]
}

fn line(link: &mut Link, offset: u8, text: &[u8]) -> Result<(), String> {
    link.show(Feedback::DisplayText { offset, text })
}

fn lcd_steps() -> Vec<Step> {
    vec![
        Step {
            watch: "both lines cleared to spaces",
            action: |link| {
                line(link, 0, &[b' '; 56])?;
                line(link, X_TOUCH.lcd_line_offset, &[b' '; 56])
            },
        },
        Step {
            watch: "'STRIP-4' at offset 21 (7 x 3) - which strip shows it, and all seven letters?",
            action: |link| line(link, 21, b"STRIP-4"),
        },
        Step {
            watch: "the upper line as one 56-character write: 1234567 eight times",
            action: |link| {
                let mut upper = [0_u8; 56];
                for (index, slot) in upper.iter_mut().enumerate() {
                    *slot = b'1' + u8::try_from(index % 7).unwrap_or(0);
                }
                line(link, 0, &upper)?;
                println!("  -> is strip 8's seventh character there? Ardour only ever sends 55");
                Ok(())
            },
        },
        Step {
            watch: "the lower line as one 56-character write at offset 0x38: ABCDEFG eight times",
            action: |link| {
                let mut lower = [0_u8; 56];
                for (index, slot) in lower.iter_mut().enumerate() {
                    *slot = b'A' + u8::try_from(index % 7).unwrap_or(0);
                }
                line(link, X_TOUCH.lcd_line_offset, &lower)
            },
        },
        Step {
            watch: "the whole 112-character buffer in one message - both lines change at once",
            action: |link| {
                let mut whole = [0_u8; 112];
                for (index, slot) in whole.iter_mut().enumerate() {
                    *slot = b"0123456789"[index % 10];
                }
                line(link, 0, &whole)
            },
        },
        Step {
            watch: "'XY' written at offset 55 - does the Y land on the lower line's first character?",
            action: |link| line(link, 55, b"XY"),
        },
        Step {
            watch: "lower case 'abcdefg' at offset 0 - the display has no lower case",
            action: |link| line(link, 0, b"abcdefg"),
        },
        Step {
            watch: "both lines cleared again",
            action: |link| {
                line(link, 0, &[b' '; 56])?;
                line(link, X_TOUCH.lcd_line_offset, &[b' '; 56])
            },
        },
    ]
}

fn color_steps() -> Vec<Step> {
    vec![
        Step {
            watch: "names and numbers on the strips, so the colours have something to light",
            action: |link| {
                let mut upper = [b' '; 56];
                for (strip, name) in [
                    b"OFF    ", b"RED    ", b"GREEN  ", b"YELLOW ", b"BLUE   ", b"MAGENTA",
                    b"CYAN   ", b"WHITE  ",
                ]
                .iter()
                .enumerate()
                {
                    let at = strip * 7;
                    upper
                        .get_mut(at..at + 7)
                        .ok_or("the row is 56 characters")?
                        .copy_from_slice(*name);
                }
                line(link, 0, &upper)?;
                line(
                    link,
                    X_TOUCH.lcd_line_offset,
                    b"0      1      2      3      4      5      6      7      ",
                )
            },
        },
        Step {
            watch: "the eight colour bytes 0..7, one per strip, left to right",
            action: |link| link.show(Feedback::DisplayColors(StripColor::ALL)),
        },
        Step {
            watch: "every strip red (byte 1)",
            action: |link| link.show(Feedback::DisplayColors([StripColor::Red; 8])),
        },
        Step {
            watch: "every strip green (byte 2)",
            action: |link| link.show(Feedback::DisplayColors([StripColor::Green; 8])),
        },
        Step {
            watch: "every strip blue (byte 4)",
            action: |link| link.show(Feedback::DisplayColors([StripColor::Blue; 8])),
        },
        Step {
            watch: "every strip white (byte 7)",
            action: |link| link.show(Feedback::DisplayColors([StripColor::White; 8])),
        },
        Step {
            watch: "every strip off (byte 0) - backlight off, text unreadable, not merely dark",
            action: |link| link.show(Feedback::DisplayColors([StripColor::Off; 8])),
        },
        Step {
            watch: "cyan everywhere, then a text write: does the text reset the colour?",
            action: |link| {
                link.show(Feedback::DisplayColors([StripColor::Cyan; 8]))?;
                std::thread::sleep(Duration::from_millis(500));
                line(link, 0, b"TEXT AFTER COLOUR                                       ")?;
                println!("  -> still cyan means text and colour are independent");
                Ok(())
            },
        },
        Step {
            watch: "four colour bytes instead of eight (sent by hand - the codec refuses)",
            action: |link| {
                link.send(&[0xF0, 0x00, 0x00, 0x66, X_TOUCH.device_id, 0x72, 1, 2, 3, 4, 0xF7])?;
                println!("  -> did the first four change and the rest hold, or nothing happen?");
                Ok(())
            },
        },
        Step {
            watch: "bit 3 set on every colour byte: 08..0F (by hand) - Xctl's inversion flag?",
            action: |link| {
                link.send(&[
                    0xF0, 0x00, 0x00, 0x66, X_TOUCH.device_id, 0x72, 0x08, 0x09, 0x0A, 0x0B, 0x0C,
                    0x0D, 0x0E, 0x0F, 0xF7,
                ])
            },
        },
        Step {
            watch: "bit 4 set: 10..17 (by hand)",
            action: |link| {
                link.send(&[
                    0xF0, 0x00, 0x00, 0x66, X_TOUCH.device_id, 0x72, 0x10, 0x11, 0x12, 0x13, 0x14,
                    0x15, 0x16, 0x17, 0xF7,
                ])
            },
        },
        Step {
            watch: "bit 6 set: 40..47 (by hand)",
            action: |link| {
                link.send(&[
                    0xF0, 0x00, 0x00, 0x66, X_TOUCH.device_id, 0x72, 0x40, 0x41, 0x42, 0x43, 0x44,
                    0x45, 0x46, 0x47, 0xF7,
                ])
            },
        },
        Step {
            watch: "nine colour bytes instead of eight (by hand)",
            action: |link| {
                link.send(&[
                    0xF0, 0x00, 0x00, 0x66, X_TOUCH.device_id, 0x72, 1, 2, 3, 4, 5, 6, 7, 1, 2,
                    0xF7,
                ])
            },
        },
        Step {
            watch: "everything back to white",
            action: |link| link.show(Feedback::DisplayColors([StripColor::White; 8])),
        },
    ]
}

/// The questions that are about what the surface does with a message it should
/// not have been sent. Every one of these is written by hand, because the codec
/// refuses to encode them — which is the point.
fn oddity_steps() -> Vec<Step> {
    vec![
        Step {
            watch: "a meter for strip 8 and strip 15 - indices this surface has not got",
            action: |link| {
                link.send(&[0xD0, (8 << 4) | METER_LEVEL_0DB])?;
                link.send(&[0xD0, (15 << 4) | METER_LEVEL_0DB])?;
                println!("  -> anything lighting on strip 1 would mean the index wraps");
                Ok(())
            },
        },
        Step {
            watch: "a ring LED message on CC 56 and CC 63 - past the eighth ring",
            action: |link| {
                link.send(&[0xB0, 56, 0x0B])?;
                link.send(&[0xB0, 63, 0x0B])
            },
        },
        Step {
            watch: "a note the table does not describe: 120, at velocity 127",
            action: |link| link.send(&[0x90, 120, 0x7F]),
        },
        Step {
            watch: "a scribble strip write on the extender's device id (0x15) - should be ignored",
            action: |link| {
                link.send(&[
                    0xF0, 0x00, 0x00, 0x66, 0x15, 0x12, 0x00, b'E', b'X', b'T', b'E', b'N', b'D',
                    b'R', 0xF7,
                ])
            },
        },
        Step {
            watch: "a device query on the extender's device id - does anything answer?",
            action: |link| link.send(&[0xF0, 0x00, 0x00, 0x66, 0x15, 0x00, 0xF7]),
        },
        Step {
            watch: "a colour message with a device id of 0x14 but only the header (no bytes)",
            action: |link| link.send(&[0xF0, 0x00, 0x00, 0x66, X_TOUCH.device_id, 0x72, 0xF7]),
        },
        Step {
            watch: "everything back to white and dark",
            action: |link| {
                link.show(Feedback::DisplayColors([StripColor::White; 8]))?;
                dark(link)
            },
        },
    ]
}
