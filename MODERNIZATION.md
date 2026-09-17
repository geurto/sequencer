# Generative Sequencer — Modernization & Portability Report

Audit date: 2026-09-17 · Last code commit: 2026-05-09 (`5a91ae2`) · Toolchain used: rustc 1.91.1

Scope: (A) bringing the Rust up to date, (B) running headless on bare-metal MCUs such as ESP32,
(C) accepting `Sequence`s from an external autoregressive model. GUI code is excluded from
recommendations but is referenced where it constrains the architecture.

---

> ### ⚠️ Implementation status — read this first
>
> **Roadmap steps 1, 2 and 3 have been implemented** — §A1 bugs, §A5 manifest, §A6 tooling, §A7
> testability, and §A4 edition 2024. Those sections describe the code *as audited*, and are kept
> because they explain why each fix looks the way it does — but they no longer describe the current
> tree. Do not re-fix them.
>
> Still outstanding and unchanged: **§A2** (concurrency model), **§A3** (platform coupling), and all
> of **Part B** and **Part C**. Those are the live work items.
>
> Step 1 landed the three mixer bugs, the silent-startup bug, the A1.5 list, unused-dependency
> removal, `rust-version = "1.87"` (verified against a real 1.87.0 toolchain), a release profile,
> a stable-only `rustfmt.toml`, GitHub Actions CI, and a Dockerfile that builds and no longer ships
> a root password.
>
> Step 2 introduced `MidiSink` and `Clock` and split the pure sequencing core (`Transport`) out of
> the platform driver (`PlaybackEngine`). **`playback/transport.rs` is the file that becomes
> `seq-core` in §B3** — it already has no channels, no threads and no input device.
>
> Step 3 moved the crate to edition 2024 and turned on `clippy::pedantic` through a `[lints]` table,
> then cleared all 91 resulting warnings. **MSRV is now 1.88** (let-chains), verified against a real
> 1.88.0 toolchain.
>
> Tests: 1 → 63 (plus 2 doctests). `cargo clippy --all-targets -- -D warnings` is clean *with
> pedantic on*. See §Changelog at the end for the detail.

---

## 0. Current state, in one page

**What it is.** A workspace-less single binary. Two `EuclideanSequencer` tasks generate 16-step
`Sequence`s, a `Mixer` folds them into a `PolyphonicSequence` (a flat `Vec<TimedEvent>` at 480 PPQN),
and a synchronous `PlaybackEngine` thread walks that event list against a wall-clock and pushes
3-byte MIDI messages through `midir`. An `iced` GUI draws state. Everything shares an
`Arc<RwLock<SharedState>>`.

**It compiles and runs.** `cargo check --all-targets` is clean apart from 5 warnings (1 unused
import, 4 `mismatched_lifetime_syntaxes` in GUI code). Default clippy adds nothing; `clippy::pedantic`
reports 117.

**The shape of the problems.** Ranked by how much they'll hurt you later:

1. Three real audio-correctness bugs in the mixer/engine hand-off (§A1) — these are almost certainly
   what "fixed sync problem" / "oops" in the log were chasing.
2. The concurrency model is *polling loops over shared mutable state*, including one uncooperative
   hot loop that pegs a core (§A2). This is also the single biggest blocker for embedded.
3. There is no separation between *musical logic* and *platform*. `sequencers/`, `mixer/` and
   `playback/` all reach for `tokio`, `iced`, `device_query` or `midir` types. Nothing can be
   compiled for a target without an OS, and nothing can be unit-tested without hardware (§A3, §B3).
4. Manifest/tooling drift: 4 unused dependencies, edition 2021, no MSRV, no lockfile, no CI, a
   `rustfmt.toml` that emits ~30 warnings per run, and a broken Dockerfile with a committed root
   password (§A5, §A6).

Points 3 is the load-bearing one: the fix for it (a `no_std` core crate) is simultaneously the
ESP32 story *and* the external-model story. Do that refactor once and B and C both fall out.

---

## Part A — Modernizing the Rust

### A1. Correctness bugs (fix these first — they're cheap and they're audible)

#### A1.1 Note-Off ticks are offset twice → overlapping notes and stuck notes

`src/mixer/mod.rs:106` (and the three sibling call sites) passes an already-offset tick:

```rust
self.add_note_off(note_a.pitch, tick_position + TICKS_PER_QUARTER_NOTE / 4 - 1, ...);
```

…and `add_note_off` at `src/mixer/mod.rs:184` adds the same offset *again*:

```rust
timed_events.push(TimedEvent {
    tick: tick_position + TICKS_PER_QUARTER_NOTE / 4 - 1,   // ← second time
    event: MidiEventType::NoteOff { pitch },
});
```

With `TICKS_PER_QUARTER_NOTE = 480`, a 16th step is 120 ticks. The Note-Off lands at
`step_tick + 238` — nearly two steps late. Consequences:

- Every note overlaps the next one (audible as legato/mud on monophonic synths).
- On the **last** step, the Note-Off tick exceeds `total_ticks`. The engine loops by resetting
  `next_event_index = 0` (`src/playback/engine.rs:170-174`), so that event is *never* processed →
  a hung MIDI note on every loop boundary.

Fix: delete the offset from one of the two places. Better, make the gate length an explicit
parameter (`gate_ticks`) instead of a magic constant repeated in four places.

#### A1.2 `PolyphonicSequence.events` is not sorted, but the engine requires it

`src/playback/state.rs:28` even says so: `pub events: Vec<TimedEvent>, // sort this by tick`.
Nothing ever sorts it. Because of A1.1 the produced order is
`[on@0, off@238, on@120, off@358, …]` — genuinely out of order.

The engine's dispatch loop (`src/playback/engine.rs:178-187`) breaks at the first event whose tick is
in the future:

```rust
while let Some(&event) = self.sequence.events.get(self.next_event_index) {
    if event.tick as f64 <= self.current_tick { self.process_event(&event); self.next_event_index += 1; }
    else { break; }
}
```

So `on@120` is blocked behind `off@238` and fires ~1 step late, and every subsequent event inherits
the skew. The same assumption is made by the resync scan at `engine.rs:81-90`.

Fix: sort in the mixer before sending (`events.sort_unstable_by_key(|e| e.tick)`), and encode the
invariant in the type — make the constructor the only way to build one:

```rust
pub struct PolyphonicSequence { events: Box<[TimedEvent]>, total_ticks: u32 }
impl PolyphonicSequence {
    pub fn new(mut events: Vec<TimedEvent>, total_ticks: u32) -> Self {
        events.sort_unstable_by_key(|e| (e.tick, e.event.order()));  // NoteOff before NoteOn at equal tick
        Self { events: events.into(), total_ticks }
    }
}
```

Note the tie-break: at equal ticks, Note-Off must precede Note-On, or a repeated pitch will be
silenced by its own predecessor's release.

#### A1.3 `num::abs_sub` is not absolute difference

`src/mixer/mod.rs:132`:

```rust
if num::abs_sub(mixer_ratio, 0.5) < 0.2 { /* emit both notes */ }
// else: emit NOTHING
```

`num_traits`' `abs_sub` is the *positive difference*: `max(x - y, 0)`, not `|x - y|`. (It's the old
`f64::abs_sub` that std deprecated for exactly this confusion.) So:

| ratio | `abs_sub(ratio, 0.5)` | branch taken |
|---|---|---|
| 0.0 | 0.0 | emits both |
| 0.3 | 0.0 | emits both |
| 0.5 | 0.0 | emits both |
| 0.7 | 0.2 | **emits nothing** |
| 1.0 | 0.5 | **emits nothing** |

The control is asymmetric, and at the top of its range coincident notes vanish entirely instead of
favouring one side. Separately, even on the working branch the ratio only gates a boolean — the
"dominant"/"accent" velocities are `random_range(60..=80)` / `random_range(40..=60)` regardless of
how far the ratio has moved. Fix the predicate to `(mixer_ratio - 0.5).abs()`, then make the ratio
map *continuously* onto velocity (or onto a per-step selection probability) so the knob does
something musical across its whole travel.

#### A1.4 Nothing is generated until the first keypress

`src/main.rs:53` calls `sequencer_a.generate_sequence();` and drops the result (the trait method
returns `Sequence`; there's no `#[must_use]`). The actual send happens in `run()`, but only when
`r_state != self.state` (`src/sequencers/euclidean/mod.rs:75`) — and at startup both the shared state
and the sequencer's local copy are `EuclideanSequencerState::new()`, so they're equal. Combined with
`pulses: 0` as the default (`state.rs:28`), the program starts silent and stays silent until a key
is pressed. Fix: emit once unconditionally on entry to `run()`, and default `pulses` to something
audible (e.g. E(4,16)).

#### A1.5 Smaller ones

| Location | Issue |
|---|---|
| `src/main.rs:74` | `midi_ports[0]` panics with an index-out-of-bounds if no MIDI port exists. Handle it and report a useful error. |
| `src/main.rs:107-113` | The Ctrl-C task is spawned *after* the blocking `Gui::run()` returns, then `main` immediately returns — it never runs. Dead code; the `ctrlc` dep is also unused. |
| `src/playback/mod.rs:56` | `run()` returns `Result<()>` but is spawned as `tokio::spawn(async move { h.run().await })`; the `Result` is dropped. The `?` on `list_ports()` inside the loop silently kills the handler. |
| `src/sequencers/euclidean/state.rs:68` | `decrease_phase` uses `saturating_sub(1) % steps`, so phase 0 stays at 0 while `increase_phase` wraps. Asymmetric. Use `(phase + steps - 1) % steps`. |
| `src/sequencers/euclidean/state.rs:73` | `(self.pitch as i8 + amount) as u8` — `u8 → i8` wraps, and the clamp happens *after*. It survives only because the clamp keeps pitch ≤ 108. Use `i16` arithmetic and clamp before the cast. |
| `src/sequencers/mod.rs:68` | `midi_to_note_name` computes `pitch - 12`, which underflows and panics for `pitch < 12`. Pitch `0` is used throughout as the "rest" sentinel. |
| `src/playback/state.rs:44-45` | `PlaybackCommand::SetBPM` / `SetMidiChannel` are matched but **never sent**. BPM is permanently 120; `SharedState::{bpm, increase_bpm, decrease_bpm}` are dead, and no key is bound to tempo. |
| `src/playback/state.rs:73-74` | `clock_ticks` and `quarter_notes` are initialized and `Debug`-printed but never written. |
| `src/sequencers/mod.rs:84` | `MixedSequence` is defined and never used anywhere. |
| `src/sequencers/mod.rs:29` | `Note::duration` is always `Sixteenth` and never read; `Note::velocity` is set to 100 and then ignored by the mixer. Two of three fields are decorative. |
| `src/playback/engine.rs:213` | `all_notes_off()` sends CC 123/120 on the current channel only. Some synths ignore CC 123; and if the channel changed, notes sounding on the old channel are orphaned. Track sounding notes in a `[u8; 16]`-indexed bitmap and send explicit Note-Offs. |

### A2. The concurrency model needs to be replaced, not patched

Three of the four long-lived tasks are polling loops over `Arc<RwLock<SharedState>>`:

- `EuclideanSequencer::run()` — `sleep(10ms)`, diff local vs shared state (`euclidean/mod.rs:65-113`).
- `Mixer::run()` — `sleep(10ms)`, same pattern (`mixer/mod.rs:39-70`).
- `PlaybackHandler::run()` — **no sleep and no unconditional await at all** (`playback/mod.rs:57-118`).
  When the three channels are empty the loop body contains zero `.await` points, so the task never
  yields to the scheduler. On the multi-thread runtime it permanently occupies one worker; on a
  single-core target (Pi Zero, ESP32) it starves everything else.
- `PlaybackEngine::run()` — `sleep(1ms)` and an X11 round-trip per iteration (§A3).

Recommended replacement:

1. **Single owner, message passing.** One task owns `SharedState`; every other component holds a
   command sender and a `tokio::sync::watch::Receiver<StateSnapshot>`. `watch` is exactly right for
   UI updates — it's lossy by design, so a slow GUI can't back-pressure the sequencer. This removes
   the `RwLock` and all "diff my copy against the shared copy" polling.
2. **`tokio::select!` instead of `try_recv` loops.** Every one of the loops above becomes
   `loop { tokio::select! { Some(cmd) = rx_cmd.recv() => …, Some(seq) = rx_seq.recv() => …, } }`.
   Zero idle CPU.
3. **Delete the `Arc<Mutex<Option<iced::futures::channel::mpsc::Sender<GuiMessage>>>>`**
   (`main.rs:39`, `playback/mod.rs:30`). That triple-wrapper exists only because the GUI's sender
   isn't available until `iced` starts. With `watch`, the GUI subscribes when it's ready and the
   producer never needs to know.
4. **Event-driven scheduling instead of a 1 ms tick.** The engine already knows the tick of the next
   event; sleep until exactly then rather than waking 1000×/s. On desktop keep a dedicated OS thread
   with `Instant`-based sleep (coarse sleep to ~1 ms out, then spin) — `tokio::time` is not
   real-time-safe. On embedded this becomes a one-shot hardware timer alarm.
5. **Integer timing.** `current_tick: f64` accumulates drift and needs an FPU. Compute event times as
   an exact rational from the bar origin:

   ```rust
   // bpm in milli-BPM so 120.0 BPM == 120_000
   let us = bar_origin_us + (tick as u64 * 60_000_000_000) / (bpm_milli as u64 * PPQN as u64);
   ```

   Recomputing from the origin each time means no accumulated error, and it's exact on a core with no
   floating-point unit (which is what the ESP32-C3 is — see §B2).

### A3. Platform coupling to remove

| Coupling | Where | Why it's a problem | Replacement |
|---|---|---|---|
| `device_query` polled inside the playback thread | `engine.rs:59, 116` | It's a **global X11 keyboard grab**. Requires a display server, works only when a window has focus, does a server round-trip at 1 kHz, and reads every keystroke the user types anywhere. Total blocker for headless. | Input as a *message source*: `crossterm` for a TUI, MIDI CC / Note-In for hardware controllers, GPIO for embedded. |
| `midir` in the engine | `engine.rs:19` | ALSA/CoreMIDI/WinMM only. | A `MidiSink` trait (§B3). `midir` is one impl; a raw-UART writer is another and works on both Pi and ESP32. |
| `iced` types in non-GUI modules | `playback/mod.rs:31`, `playback/state.rs` via `SharedState` | The playback layer imports `iced::futures::channel::mpsc`, so `playback` cannot build without a GUI stack. | `watch` channel of a plain snapshot type. |
| `tokio` in the sequencers/mixer | throughout | `tokio` is `std`-only. | Make the generators synchronous and pure (§B3). Async belongs at the I/O edge. |
| `log`/`env_logger` in core | throughout | `env_logger` is `std`. `log` itself is fine. | Keep the `log` facade in core; choose the backend per binary (`tracing-subscriber` on desktop, `defmt`/`esp-println` on MCU). |
| `anyhow` in library code | `playback/`, `midi.rs` | Opaque errors in a library; `std` by default. | `thiserror` 2.0 enums in the library (works on `no_std` via `core::error::Error`, stable since 1.81); keep `anyhow` in `main.rs` only. |

### A4. Edition 2024 and language-level modernization

Move `edition = "2021"` → `"2024"` (Rust ≥ 1.85). Migration notes specific to this code:

- **`impl Future` in traits.** `Sequencer::run()` is hand-desugared as
  `fn run(&mut self) -> impl Future<Output = ()> + Send` (`sequencers/mod.rs:7`) — the usual
  workaround for AFIT not implying `Send`. Edition 2024 changes RPIT lifetime capture, so re-check
  this signature during migration. The better answer is to **delete async from the trait entirely**
  and make generators synchronous (§C1) — that fixes the `Send` problem, the `no_std` problem and
  the testability problem at once. If you must keep async traits, use `trait_variant::make`.
- `unsafe_op_in_unsafe_fn` becomes deny-by-default and `static mut` references become hard errors —
  neither appears here, so migration should be mechanical. `cargo fix --edition` will handle the rest.

Other modern idioms worth adopting:

- **Let-chains** (stable 1.88, edition 2024) collapse the nested `if let` pairs at
  `playback/mod.rs:106-114` and `242-250`.
- **`#[expect(lint)]`** (1.81) instead of `#[allow]` — it warns when the lint stops firing, so
  suppressions don't rot.
- **Inline format args** — clippy flags 9 sites still doing `format!("{}", x)`.
- **`#[derive(PartialEq)]`** — the hand-written `impl PartialEq for EuclideanSequencerState`
  (`euclidean/state.rs:85-92`) is byte-for-byte what the derive produces. Same for the manual
  `Debug` for `SharedState`, which clippy notes omits fields (`current_note_index`).
- **`Cargo.toml [lints]` table** (stable 1.74) — configure once in the manifest instead of
  `#![warn(...)]` in `lib.rs`:

  ```toml
  [lints.rust]
  unsafe_code = "forbid"
  missing_debug_implementations = "warn"

  [lints.clippy]
  pedantic = { level = "warn", priority = -1 }
  cast_possible_truncation = "warn"
  cast_sign_loss = "warn"
  ```

  The pedantic run found 117 warnings, including the `u8 as i8` (A1.5), the `f64 as usize`
  truncations in the engine, and `unused_async` on `Mixer::mix` (`mixer/mod.rs:73` is `async` with no
  `.await` in its body).

### A5. Manifest and dependency hygiene

```toml
[package]
name = "sequencer"
version = "0.1.0"
edition = "2024"
rust-version = "1.88"    # ← currently absent; the code already needs ≥1.87
                         #   for usize::is_multiple_of (mixer/mod.rs:79)
```

**Remove — unused, zero references in `src/`:** `markov`, `throttle`, `rustc-hash`, `ctrlc`.
(`markov` is presumably the ghost of the "+AI sequencer" roadmap item; see Part C for where that
should actually live.) Add `cargo-machete` or `cargo-udeps` to CI so this doesn't recur.

**Upgrades available:** `iced` 0.13.1 → 0.14.0 (out of scope but note the `Element<'_, T>` lifetime
warnings are 0.14-era API hygiene), `midir` 0.10 → 0.11. `tokio`, `rand` 0.9, `env_logger` 0.11,
`anyhow` are current.

**Add a release profile** — the defaults are wrong for a real-time audio binary:

```toml
[profile.release]
lto = "thin"
codegen-units = 1
panic = "abort"
```

**Commit `Cargo.lock`.** `.gitignore` currently excludes it. For a binary crate the lockfile should
be tracked — it's what makes a build reproducible on the Pi three months from now. (`.gitignore`
also lists `flake.lock`, which *is* tracked, so that line is misleading; drop it.)

**Add `rust-toolchain.toml`** pinning the channel and the extra targets — this becomes mandatory once
you add an embedded target.

### A6. Tooling, CI and repo hygiene

- **`rustfmt.toml` is a dumped default config.** It sets ~30 nightly-only options, so every
  `cargo fmt` prints ~30 `can't set ... unstable features are only available in nightly` warnings.
  It also declares `edition = "2015"` / `style_edition = "2015"`, which is wrong for a 2021/2024 crate
  and affects how rustfmt parses the source, and `required_version = "1.8.0"`, which will hard-fail
  on a future rustfmt. Replace the whole file with the handful of options you actually care about:

  ```toml
  edition = "2024"
  style_edition = "2024"
  max_width = 80
  ```

- **No CI.** Add a GitHub Actions workflow: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test`, plus (once §B lands) `cargo build -p sequencer-esp --target riscv32imc-unknown-none-elf`.
  Add `cargo-deny` for advisories/licences.
- **Only one test exists** (`test_pitch_to_note`). See §A7.
- **`docker/sequencer/Dockerfile` is broken and unsafe.**
  - `COPY src/entrypoint.sh /entrypoint.sh` — the file is at the repo root, not in `src/`. The build
    fails.
  - `RUN #cargo build --release` is a comment inside a `RUN`, i.e. a no-op layer.
  - It hardcodes `root:BigJuice420`, then sets `PermitRootLogin yes` and `PasswordAuthentication yes`.
    A committed root password in a git-tracked Dockerfile is a credential leak regardless of how the
    image is used. Remove sshd from the image entirely (use `docker exec`), or at minimum switch to
    key-based auth injected at runtime. Treat that password as burned.
- **`ttymidi.tar.gz` (76 KB) is a vendored binary blob in git.** Fetch it in the Dockerfile instead,
  or — better — bypass it: write MIDI bytes straight to `/dev/serial0` with the `serialport` crate
  (it supports the non-standard 31250 baud on Linux). That deletes the ALSA dependency on the Pi
  *and* gives you the exact code path you'll reuse on ESP32.

### A7. Testability — the highest-leverage structural change

Right now nothing about timing or MIDI output can be tested: the engine owns a real
`MidiOutputConnection`, reads a real `Instant::now()`, and polls a real keyboard. Two injections fix
this:

```rust
pub trait MidiSink { fn send(&mut self, msg: &[u8]) -> Result<(), MidiError>; }
pub trait Clock { fn now_us(&self) -> u64; }
```

With a `Vec`-backed `MidiSink` and a manually-advanced `Clock`, the entire engine becomes a pure
function of (sequence, clock advances) → (list of MIDI bytes). Then:

- Golden tests for Euclidean generation: E(3,8) == `x..x..x.`, E(5,8) == `x.xx.xx.`, etc.
- **A property test that Note-Ons and Note-Offs balance over N loop iterations** — this alone would
  have caught A1.1.
- **A property test that `events` is sorted by tick** — catches A1.2.
- A test that mixer ratio 0.0 and 1.0 both produce sound — catches A1.3.
- Fuzzing (`cargo-fuzz` / `arbitrary`) on the wire decoder once Part C lands.

---

## Part B — Running on bare metal (ESP32)

### B1. Verdict

**Yes, comfortably — but not by porting this code. By extracting a `no_std` core from it.**

The musical workload is trivial for an MCU: a 16–144 step pattern is a couple of hundred bytes, the
Bresenham/Euclidean generator is integer arithmetic over ≤ 64 elements, and MIDI DIN is a 31250-baud
byte stream. A 2003-era microcontroller could do this. What *doesn't* port is the plumbing: `tokio`,
`midir`, `device_query`, `iced`, `Vec`, `f64` timing, `anyhow`, `env_logger`.

So the work is not "make it smaller", it's "stop the musical logic from depending on an OS". That
refactor (§B3) is worth doing even if you never ship an ESP32.

### B2. Chip and toolchain choice — this decision comes first

`esp-hal` reached **1.0 stable** and is now at 1.2.1 (MSRV 1.88). `esp-hal-embassy` 0.9,
`esp-wifi` 0.15, `embassy-executor` 0.10. The bare-metal Rust story on Espressif is genuinely good
now. But the toolchain differs by architecture:

| Family | ISA | rustup target | Toolchain | FPU | Native USB |
|---|---|---|---|---|---|
| ESP32, ESP32-S2, ESP32-S3 | Xtensa | `xtensa-esp32*-none-elf` — **tier 3, not rustup-installable** | needs `espup` (LLVM/Rust fork) or nightly `-Zbuild-std` | S3: yes | S2/S3: USB-OTG ✓ |
| ESP32-C2, C3 | RISC-V RV32IMC | `riscv32imc-unknown-none-elf` — **tier 2, `rustup target add`** | plain stable Rust ✓ | **no** | USB-Serial-JTAG only ✗ |
| ESP32-C6, H2 | RISC-V RV32IMAC | `riscv32imac-unknown-none-elf` — **tier 2** | plain stable Rust ✓ | **no** | USB-Serial-JTAG only ✗ |

(Verified against `rustc --print target-list` and `rustup target list` on 1.91.1: the Xtensa triples
exist in rustc's target list but have no distributed `core`, so they need espup or `build-std`.)

**Recommendation: ESP32-C6 with DIN/TRS MIDI out.** Stable upstream toolchain, no vendor Rust fork,
`riscv32imac` is a tier-2 target, 512 KB SRAM, and WiFi 6 + Thread for the Part C link. The missing
FPU is a non-issue once timing is integer (§A2.5) — and it's a good forcing function to get that
right.

**Choose ESP32-S3 instead if class-compliant USB-MIDI matters to you.** The C3/C6 USB peripheral is
Serial/JTAG-only and cannot enumerate as a USB-MIDI device; the S3's USB-OTG can (`embassy-usb` +
a MIDI class descriptor). The cost is the `espup` toolchain and the Xtensa fork. You get an FPU and
optional PSRAM in exchange. ESP32-P4 would be the ideal combination (RISC-V + USB 2.0 OTG) but it's
not in `esp-hal` 1.x's supported chip list yet — worth re-checking before you commit.

**A note on `esp-idf-hal` / `std`.** Espressif also ships a `std` toolchain
(`riscv32imc-esp-espidf`) giving you `std`, threads and a subset of `tokio`. That would let you port
far more of the existing code verbatim. It's a legitimate shortcut, but it drags in the whole ESP-IDF
C stack and its build system, and you'd inherit the polling architecture rather than fixing it. I'd
use it only as a spike to prove the hardware, not as the destination.

### B3. The enabling refactor: a workspace with a `no_std` core

```
sequencer/
├── Cargo.toml                 # [workspace]
├── crates/
│   ├── seq-core/              # #![no_std], no alloc, no I/O, no async. The music.
│   │   ├── note.rs            #   Pattern, Step, Voice, Scale
│   │   ├── euclid.rs          #   Euclidean generator (pure fn)
│   │   ├── mixer.rs           #   Pattern × Pattern → Pattern (pure fn)
│   │   ├── engine.rs          #   tick-driven state machine: (tick) -> &[MidiEvent]
│   │   └── source.rs          #   trait SequenceSource  ← Part C hooks here
│   ├── seq-io/                # #![no_std] traits: MidiSink, Clock, ControlInput, Transport
│   ├── seq-proto/             # #![no_std] + serde wire types (Part C), heapless/alloc feature-gated
│   ├── sequencer-desktop/     # std bin: midir + tokio + iced + serialport
│   └── sequencer-esp/         # no_std bin: esp-hal + embassy + UART MIDI + WiFi
```

The rules that make this work:

1. `seq-core` is `#![no_std]` and **allocation-free**. Fixed-capacity types, `Copy` everywhere.
2. `seq-core` contains **no async and no channels**. The engine is a state machine you *call*:
   `fn advance_to(&mut self, now_us: u64) -> impl Iterator<Item = MidiEvent>`. Who calls it, and
   from what executor, is the platform's business.
3. All I/O is behind `seq-io` traits. Both binaries are thin: wire up peripherals, then loop.
4. `seq-core` gets a dev-dependency on `proptest` and carries the tests from §A7. It's the only
   crate with interesting logic and it runs on the host in milliseconds.

Core types, sized for an MCU:

```rust
pub const MAX_STEPS:  usize = 64;
pub const MAX_VOICES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Voice { pub pitch: u8, pub velocity: u8, pub gate_ticks: u16 }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Step { pub voices: [Option<Voice>; MAX_VOICES] }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub steps: [Step; MAX_STEPS],
    pub len: u16,             // active steps
    pub ticks_per_step: u16,
}
```

`Pattern` is ~1 KB and `Copy` — pass it by value, hand it between tasks with no allocator. Note this
also fixes the "pitch 0 means rest" sentinel (A1.5): a rest is `None`, so MIDI note 0 stays a real
note.

### B4. Dependency substitution table

| Desktop | Embedded | Notes |
|---|---|---|
| `tokio` | `embassy-executor` 0.10 + `esp-hal-embassy` 0.9 | Or no executor at all — a bare `loop` with a timer is enough for a sequencer. |
| `tokio::sync::{mpsc,RwLock,watch}` | `embassy-sync::{Channel, Signal, Mutex}` | `Signal` is the embedded equivalent of `watch` — lossy latest-value. |
| `std::time::Instant` | `embassy_time::Instant` (µs resolution) | Or the raw SYSTIMER for the hard real-time path. |
| `midir` | `esp-hal::uart::Uart` @ 31250 8N1 | Same `MidiSink` trait on both sides. |
| `Vec<T>` | `heapless::Vec<T, N>` 0.9 / fixed arrays | `heapless` MSRV 1.87. |
| `f32`/`f64` timing | `u64` µs integer math (§A2.5) | Mandatory on C3/C6 — no FPU. |
| `rand` 0.9 | `rand_core` + `rand_xoshiro` 0.8, or `esp_hal::rng::Rng` (hardware TRNG) | The mixer's `random_range` needs replacing. |
| `num::integer::lcm` | `num-integer` with `default-features = false`, or 6 lines of inline gcd/lcm | Not worth a dependency. |
| `anyhow` | `thiserror` 2.0 / plain enums | `core::error::Error` is stable. |
| `log` + `env_logger` | `log` + `esp-println` 0.18, or `defmt` | Keep the `log` facade in core so both work. |
| `device_query` | GPIO + rotary encoders, or MIDI-In CC | See §B6. |
| `serde_json` | `postcard` 1.1 / `serde-json-core` 0.6 | Part C. |

### B5. MIDI output on the ESP32

**DIN / TRS-A (recommended).** MIDI is 31250 baud, 8N1 — the ESP32 UART divider handles it exactly.
Drive the TX pin through a buffer and the standard 3.3 V resistor pair (33 Ω / 10 Ω per the MIDI 1.0
electrical spec's low-voltage update) into a 5-pin DIN or a TRS Type-A jack. For MIDI **In**, you
need a 6N138/H11L1 optocoupler — don't skip it, it's what keeps ground loops out of your rack.

At 31250 baud a 3-byte message takes ~0.96 ms. That's your real jitter floor and it's also why you
should **not** share this UART with the Part C control link: a 270-byte pattern frame would occupy
the bus for 86 ms. Use a second UART (or WiFi) for control.

**USB-MIDI.** Only on S2/S3 (USB-OTG) via `embassy-usb`. C3/C6 cannot.

**Timing strategy.** Don't poll. Compute the absolute µs of the next event and arm a one-shot
`embassy_time::Timer::at()`, or run the dispatch in a high-priority timer ISR. Target < 1 ms jitter;
the transmit time above means there's no point chasing better than that.

### B6. Input without a keyboard

`device_query` has to go regardless (§A3). For a headless box the natural inputs are:

- **Rotary encoders + buttons on GPIO**, debounced, read via `embassy-time` or PCNT. Maps 1:1 onto
  the existing key semantics (steps/pulses/phase/pitch per slot).
- **MIDI In CC** — makes the sequencer controllable from any DAW or control surface, and gives you
  MIDI clock sync for free (see below).
- **WiFi/BLE control** — reuse the Part C transport for parameter changes, not just patterns.

Model all three as `enum ControlEvent { … }` feeding one command channel. That's the same enum the
desktop TUI produces, so the core never learns where input came from.

### B7. Also worth adding while you're in there: MIDI clock

The sequencer currently has no external sync and emits no clock. For hardware use that's a
significant gap — it can't lock to a drum machine or a DAW. Both directions are cheap:

- **Out:** send `0xF8` 24× per quarter note, plus `0xFA`/`0xFC` on start/stop.
- **In:** count `0xF8`, derive BPM from the interval, and drive the engine from external ticks
  instead of the internal clock.

The `clock_ticks` / `quarter_notes` fields in `SharedState` (currently never written, A1.5) look like
this was already the plan.

### B8. Resource budget (ESP32-C6, 512 KB SRAM, 4 MB flash)

| Item | RAM |
|---|---|
| `Pattern` × 4-deep lookahead ring | ~4 KB |
| Wire RX/TX buffers | ~2 KB |
| Sounding-note tracking, engine state | < 1 KB |
| embassy executor + 4 task stacks | ~16 KB |
| **Subtotal, no WiFi** | **~25 KB** |
| `esp-wifi` + `embassy-net` (if used) | ~60–80 KB |

Firmware size lands around 200–400 KB of 4 MB. There is no resource problem here — the constraint is
architectural, not physical.

---

## Part C — Receiving Sequences from an external source

This is where the design gets interesting, and where the current architecture would fight you: the
`Sequencer` trait (`sequencers/mod.rs:5`) bakes in `async`, `Send`, and a private `tokio` channel, so
"a sequencer" can only ever be an always-ready in-process tokio task. An autoregressive model is
none of those things — it's **remote, slow, and allowed to fail**.

### C1. The abstraction: a pull-based, non-blocking, `no_std` source trait

```rust
// seq-core/src/source.rs
#[derive(Clone, Copy, Debug)]
pub struct GenContext {
    pub bar: u64,
    pub steps_per_bar: u16,
    pub bpm_milli: u32,
    pub root: u8,          // MIDI pitch of the tonic
    pub scale: Scale,
    pub density: u8,       // 0..=255 macro control (maps to the current mixer ratio)
    pub deadline_us: u64,  // absolute time by which a pattern is useless
}

pub trait SequenceSource {
    /// MUST NOT block, allocate, or await. `None` = nothing ready yet.
    fn try_next(&mut self, ctx: &GenContext) -> Option<Pattern>;

    /// What actually played. Conditioning signal for stateful/learned sources.
    fn observe(&mut self, _bar: u64, _played: &Pattern) {}
}
```

Three properties make this the right shape:

- **Non-blocking and synchronous** → identical on tokio and embassy, works in an ISR, works in
  `no_std`, and is trivially unit-testable.
- **Pull, not push** → the engine asks when *it* is ready, at a musically safe moment. A push model
  lets the model dictate timing, which is exactly backwards.
- **`observe`** → makes conditioning a first-class part of the contract rather than an afterthought.

Implementations:

| Impl | Notes |
|---|---|
| `EuclideanSource` | The existing generator, made pure. Always returns `Some`. |
| `MarkovSource` | On-device order-2/3 chain over step patterns. A few KB of `u8` transition tables. (This is what the orphaned `markov` dep was for.) |
| `RemoteSource<T: Transport>` | Owns a decode buffer and a lookahead ring; `try_next` pops a frame if one arrived. |
| `FallbackSource<A, B>` | **The key combinator.** `A.try_next().or_else(|| B.try_next())`. Wrap the remote source in this with a Euclidean fallback and a missed model deadline degrades to "keeps playing" instead of "stops". |
| `LatchSource<S>` | Repeats the last pattern when the inner source is dry. The gentler fallback. |

### C2. Real-time safety — the part that actually matters

An autoregressive model has unbounded, highly variable latency (tens of ms to seconds, plus network).
The playback path must never, under any circumstance, wait on it.

```
   bar N-1        bar N          bar N+1        bar N+2
  ───────────┬──────────────┬──────────────┬──────────────
             │
  at bar N start: emit GenRequest{ bar: N+2, deadline: end of bar N+1 }
             │
             └──── model thinks (network + inference) ────┐
                                                          ▼
                                            PatternFrame{ bar: N+2 } arrives
                                                          ▼
                                     lookahead ring [N+1, N+2] ← decoded here
                                                          ▼
                    engine swaps patterns ONLY at a bar boundary
```

Rules:

1. **Two-bar lookahead.** Request bar N+2 at the start of bar N. At 120 BPM a 16-step bar of 16ths is
   2 s, so the model gets ~4 s of budget. That's generous for a small transformer on a host CPU and
   workable over WiFi.
2. **Swap only at bar boundaries.** The current code swaps immediately on `LoadSequence` and rescales
   `current_tick` by a length ratio (`engine.rs:74-101`) — that's the machinery that produces the
   mid-bar discontinuities and orphaned Note-Offs. Delete it. Latch the pending pattern; adopt it when
   the bar wraps.
3. **Always release sounding notes before a swap.** Keep a `[u128; 16]` (channel × pitch) bitmap of
   what's sounding and emit explicit Note-Offs. Don't rely on CC 123 (A1.5).
4. **Deadline + fallback, never a stall.** If nothing arrived by the boundary: `LatchSource` repeats
   the last bar, or `FallbackSource` drops to Euclidean. Log it, surface it in the UI, keep playing.
5. **Decode off the audio path.** Deserialize in the network task into the lookahead ring; the engine
   only ever does a `Copy` out of a `Signal`/slot. No allocation, no parsing, no locking in the
   playback thread.
6. **Bounded queues everywhere, drop-oldest.** A model that runs fast must not be able to queue up
   30 s of stale future.

### C3. Wire format

One `seq-proto` crate, `#![no_std]`, `serde` derives, with the encoder feature-gated per platform:
`serde_json` on desktop for debuggability, `postcard` on the wire for compactness. Same types both
ways, so a desktop integration test can drive the embedded decoder.

```rust
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct WireNote { pub step: u8, pub pitch: u8, pub vel: u8, pub gate_steps: u8 }  // 4 bytes

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PatternFrame {
    pub schema: u8,             // version — bump on breaking change, reject unknown
    pub bar: u64,               // which bar this is FOR; late frames get dropped
    pub steps: u8,
    pub ticks_per_step: u16,
    pub notes: Vec<WireNote>,   // heapless::Vec<WireNote, 256> under no_std
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GenRequest {
    pub schema: u8,
    pub bar: u64,
    pub deadline_ms: u32,
    pub ctx: WireContext,           // bpm_milli, root, scale, density, steps_per_bar
    pub history: Vec<PatternFrame>, // last K bars ACTUALLY PLAYED — the conditioning prompt
}
```

64 notes ≈ 270 bytes in postcard. Frame it with COBS — `postcard::to_slice_cobs` /
`take_from_bytes_cobs` gives you self-synchronizing framing for free over a UART, which matters
because a byte-oriented link will eventually desync. Over TCP, length-prefix instead.

Two rules that will save you pain:

- **The `bar` field is the contract.** A frame for a bar that already played is silently dropped.
  This makes the whole thing robust to a slow model without any coordination protocol.
- **The firmware only ever speaks `PatternFrame`.** Model tokenization, vocabulary, sampling
  temperature, key detection — all of that lives in an adapter next to the model. If you later swap
  an Anticipatory Music Transformer for MusicGen or a hand-rolled RNN, nothing downstream changes.

### C4. Where the model runs, and how it connects

| Placement | Transport | Latency | When |
|---|---|---|---|
| Host PC (Python/PyTorch), sequencer on Pi or desktop | TCP or Unix socket, postcard/JSON | 10–500 ms | Best quality. Start here. |
| Same process, Rust inference | `candle` 0.11 (pure Rust, no C++ build) or `ort` 2.0 (ONNX Runtime) | 5–100 ms | Good on a Pi 4/5. Single binary, no Python. |
| Host PC → ESP32 | WiFi TCP (`embassy-net`) or MQTT (`minimq` 0.13) | 20–200 ms + jitter | The standard embedded setup. |
| Host PC → ESP32 | Second UART, 115200+, COBS-framed | ~25 ms for a frame | No WiFi stack (saves ~70 KB RAM), fully deterministic. Good for a spike. |
| **On the ESP32 itself** | none | µs | Order-2/3 Markov chain over 16-step patterns: a few KB of tables, entirely feasible. A quantized int8 GRU via `microflow`/tflite-micro is also plausible at ~10–50 k params. A transformer is not. |

**Recommended end state:** the ESP32 runs `FallbackSource<RemoteSource<WifiTransport>, MarkovSource>`
with `EuclideanSource` as the floor. It's musically autonomous standalone, and gets smarter when a
host is on the network. Same trait, three impls, no conditional compilation in the engine.

### C5. Conditioning — make the link bidirectional from day one

An autoregressive model that can't see what was played is just a random pattern generator with extra
latency. `GenRequest.history` carrying the last K bars *as actually played* (post-mixer, post-fallback)
is what turns this into a continuation model. Two details:

- Send what was **played**, not what was requested — otherwise a deadline miss silently corrupts the
  model's context.
- Send the macro parameters too (`bpm`, `root`, `scale`, `density`). The existing mixer ratio is a
  natural `density`/`complexity` conditioning axis, and it gives the knob a real job (cf. A1.3).

### C6. Sketch: wiring it into the existing engine

```rust
// In the desktop binary
let remote = RemoteSource::connect("127.0.0.1:7777", LOOKAHEAD_BARS)?;
let mut source = FallbackSource::new(remote, LatchSource::new(EuclideanSource::new(state)));

// In the engine's bar-boundary handler — no allocation, no await, no lock
if self.bar_wrapped() {
    let ctx = self.gen_context();
    if let Some(next) = source.try_next(&ctx) {
        self.release_sounding_notes();
        self.pending = Some(next);
    }
    source.observe(self.bar, &self.current);
    self.adopt_pending();
}
```

The engine has no idea whether that pattern came from Bresenham, a Markov table, or a 300 M-parameter
transformer three hops away. That's the whole point.

---

## Suggested order of work

Each step is independently shippable and leaves the tree working.

1. ~~**Bug fixes + hygiene** (§A1, §A5, §A6).~~ **Done** — see §Changelog.
2. ~~**Testability** (§A7).~~ **Done** — see §Changelog.
3. ~~**Edition 2024 + `[lints]`** (§A4).~~ **Done** — see §Changelog.
4. **Workspace split, extract `seq-core` as `no_std`** (§B3). The big one. Integer timing (§A2.5),
   fixed-capacity `Pattern`, synchronous pure generators. Desktop binary keeps working throughout.
5. **Replace the concurrency model** (§A2). `watch` + `select!`, delete the polling loops and the
   `Arc<Mutex<Option<Sender>>>`. Drop `device_query`.
6. **`seq-proto` + `SequenceSource` + lookahead/fallback** (§C1–C3), with a mock source first, then a
   real TCP one against a Python stub. Part C is now done on desktop.
7. **`sequencer-esp`** (§B). Blink → UART MIDI out → Euclidean standalone → GPIO input → WiFi remote
   source. Each stage is a working instrument.
8. **MIDI clock in/out** (§B7), on-device `MarkovSource` (§C4), then the real model.

Steps 1–3 are a weekend. Step 4 is the real investment, and it's the one that makes 6, 7 and 8
straightforward instead of painful.

---

## Changelog — roadmap step 3 (implemented)

### Edition 2024

`cargo fix --edition` had **nothing to change** — the migration was clean, and `cargo check` came
back with zero warnings on the first build under 2024. Two things the audit flagged as risks were
checked and turned out not to bite:

- **The `impl Future` in the `Sequencer` trait.** Edition 2024's RPIT capture change affects
  return-position `impl Trait` in free and inherent functions; RPIT *in traits* already captured all
  lifetimes in 2021, so the hand-desugared `fn run(&mut self) -> impl Future<Output = ()> + Send`
  needed no change. (It should still go away — see §A4's note on making generators synchronous.)
- **`tail_expr_drop_order`.** The migration lint flagged four sites where a temporary now drops
  before the block's locals: three in `playback/mod.rs`, one in `playback/engine.rs`. All four are
  `while let`/tail-expression cases where the value is either moved out of the temporary, or is a
  lock released at the same function boundary either way. The one worth naming is
  `update_gui`, where a `MutexGuard` temporary now drops *earlier* — strictly an improvement, since
  it shortens the lock hold. No `Drop` in the affected set sends messages or closes a port at a
  point where ordering is observable; the sink is closed inside `Transport::set_sink`, after the
  release, which the change does not touch.

Also: `unsafe_code = "forbid"` (there was none), `static mut` (none), the `gen` keyword (none).

### `[lints]` table and pedantic cleanup

Configured in `Cargo.toml` rather than `lib.rs`, so it applies to the binary and tests too. Turning
on `clippy::pedantic` produced 91 warnings; `cargo clippy --fix` took 49 mechanically (inline format
args, `#[must_use]`, missing semicolons, redundant closures), and the remaining 42 were done by
hand. `missing_errors_doc`, `missing_panics_doc` and `module_name_repetitions` are waived — not
worth it for a binary crate with one library consumer. The casting lints are explicitly kept on
rather than waived with the rest of pedantic: a wrong cast in the timing or MIDI path is silent and
audible.

Most casts were removed rather than suppressed:

| Site | Before | After |
|---|---|---|
| `Transport::sounding_notes` | `enumerate()` then `pitch as u8` | zip over `0..=127u8` — no cast |
| `Transport::release_all_notes` | index `usize`, `pitch as u8` | iterate `u8`, `usize::from` to index |
| `SystemClock::now_us` | `as_micros() as u64` | `u64::try_from(..).unwrap_or(u64::MAX)` — total |
| `Mixer::mix` | `sequence_length as u32` | bounded by a new `MAX_SEQUENCE_STEPS`, then `u32::from` |
| `mixer::voice` | `as i16` then `as u8` | one documented `#[expect]`, then `u8::try_from(..).ok()?` |
| `change_pitch` | `clamp(..) as u8` | `u8::try_from` after the clamp |
| GUI grid drawing | `usize as f32` | loop over `u16`, `f32::from` — no cast |

Four casts genuinely cannot go — float→int has no `From` — and each now carries an `#[expect]` with
a `reason` explaining why it is sound. There are **no `#[allow]` attributes left in the crate**;
`#[expect]` is used throughout, so a suppression that stops being needed becomes a warning instead
of rotting silently.

The `MAX_SEQUENCE_STEPS` bound in the mixer is new behaviour, not just a cast fix: two sequencers
capped at 16 steps can only ever need lcm(16, 15) = 240 steps, so anything past 4096 is an upstream
bug, and building it would allocate an event list nobody asked for. It now logs and emits silence.

### Language-level modernization

- **Let-chains** (the reason MSRV is 1.88): both sites the audit identified — in
  `handle_midi_command` and `update_gui` — are now single `if let ... && let ...` conditions rather
  than nested `if let`s. Clippy's `collapsible_if` found them automatically once the edition changed.
- **`handle_input_change` lost ~80 lines.** `too_many_lines` pointed at it, and the fix was not
  cosmetic: ten keyboard arms were each hand-rolling a
  `match active_sequencer { Left => left.foo(), Right => right.foo() }` dispatch that `SharedState`
  already had methods for. Those methods were dead code left from the original design. They are now
  the only path, so the active-sequencer dispatch exists in exactly one place.
  `increase_phase`/`decrease_phase` were the two that were missing; they have been added.
- **`Debug` everywhere** (`missing_debug_implementations`). Eight types took a plain derive. Three
  needed hand-written impls because they hold trait objects or unbounded type parameters —
  `Transport` prints the musical state (playing, bpm, step, tick, event count, notes sounding),
  `PlaybackEngine` forwards to it, and `PlaybackCommand` prints the variant plus an event count
  rather than dumping a whole sequence into a log line.
- **rustfmt `style_edition = "2024"`**, which reorders imports by version-sort. 27 files touched,
  import ordering only.

### Notes

- Test-only cast and `float_cmp` warnings were fixed rather than blanket-allowed — `try_from(..)
  .unwrap()` in test helpers, and two `#[expect(clippy::float_cmp)]` where the compared values are
  exact by construction (a clamp returns its bound; the BPM is stored verbatim).
- One lint is waived per-module with a reason rather than crate-wide: `unreadable_literal` in
  `gui/theme.rs`, because splitting `0xcba6f7` into digit groups makes a colour harder to read
  against the palette it came from.

---

## Changelog — roadmap step 2 (implemented)

### The seam (§A7)

`playback/engine.rs` used to hold the play position, the event cursor, a real `MidiOutputConnection`,
a real `Instant`, a command channel and an X11 keyboard grab, all in one `run()` loop. It is now two
pieces:

- **`playback/transport.rs` — `Transport<S: MidiSink>`.** The sequencing core: play position, event
  dispatch, note bookkeeping. No channels, no threads, no input device, no `Instant`. Driven purely
  by `advance_to(now_us)`, which makes playback a function of (sequence, clock readings) → MIDI
  bytes. **This is the file that becomes `seq-core` in §B3.**
- **`playback/engine.rs` — `PlaybackEngine<C: Clock>`.** The platform driver: command channel,
  keyboard poll, sleep, and the `NotePlayed` status feed. Its loop body is factored into `update()`
  so everything except the keyboard poll can be driven under test.

New traits, both with a real implementation and a test double:

| Trait | Production | Test double |
|---|---|---|
| `MidiSink` (`playback/sink.rs`) | `MidiOutputConnection`, plus a blanket impl for `Box<T>` | `RecordingSink` — captures messages; clones share one recording, so a test keeps a handle after handing the sink over. `RecordingSink::failing` exercises error paths. |
| `Clock` (`playback/clock.rs`) | `SystemClock` (`Instant`-based) | `ManualClock` — moves only when told; clones share one timeline. |

`SendError` mirrors `midir::SendError` but names no backend types and carries `&'static str` rather
than `String`, so it stays allocation-free for the eventual `no_std` core.

`PlaybackCommand::SetOutputConnection` now carries a `BoxedSink` rather than a `MidiOutputConnection`
— the output is swapped at runtime, so it cannot be a type parameter, and boxing is what keeps the
command type free of midir.

### Simplifications that fell out

- **`next_note_tick` is gone.** The GUI play head was driven by a second counter kept alongside the
  play position, adjusted by `-= total_ticks` on loop and `.ceil()` on load. It is now derived:
  `Transport::current_step()`, with the engine reporting on change. One less thing to desync.
- **The engine's duplicate `midi_channel` handling is gone** — `set_midi_channel` is the only path.
- **Long stalls no longer burst.** `wrap_around` discards whole laps (`floor(tick / total)`) instead
  of subtracting one length, so a stalled thread cannot leave the head past the end and replay the
  entire sequence on each following call.
- **Loading a sequence re-seeks the event cursor** (`partition_point`), so a swap to a
  different-length sequence resumes at the rescaled head instead of replaying everything up to it.

### Tests: 21 → 63 (plus 2 doctests)

Every new behavioural test was checked by mutation — reintroducing the behaviour it guards and
confirming it fails:

| Mutation | Caught by |
|---|---|
| Pause stops releasing held notes | `test_pause_releases_sounding_notes`, `test_notes_balance_over_many_loops` |
| Wrap subtracts one lap instead of all | `test_stall_leaves_the_head_inside_the_sequence` |
| Load skips the cursor re-seek | `test_loading_a_longer_sequence_does_not_burst` |
| Pitch/velocity masking removed | `test_out_of_range_pitch_and_velocity_are_masked` |
| Channel change skips the release | `test_channel_change_releases_on_the_old_channel` |
| Step reporting loses its dedup | `test_each_step_is_reported_exactly_once` |
| `SetBPM` / `SetOutputConnection` dropped | `test_commands_reach_the_transport`, `test_switching_output_moves_playback_to_the_new_sink` |

Property tests (`proptest`, a dev-dependency — it does not enter the shipped binary):

- `prop_stopping_never_leaves_a_note_hanging` — for arbitrary event lists (unbalanced, out of order,
  past the end) and arbitrary clock behaviour (stalls, repeats, going backwards), stopping must
  leave nothing ringing. A hung MIDI note is the worst failure this program has.
- `prop_play_head_stays_inside_the_sequence`.
- `prop_mixed_sequences_release_every_note`, `prop_mixed_events_are_sorted_and_in_range`,
  `prop_velocities_are_audible_and_in_range`.
- `prop_new_always_sorts` — the `PolyphonicSequence` invariant from step 1.

Writing the ringing check surfaced a modelling subtlety worth keeping in mind: counting Note-On/Off
pairs is the *wrong* model for arbitrary input, because a looping sequence can re-strike a held pitch
and a synth has only one voice per pitch, so one Note-Off correctly releases several Note-Ons. The
property models held notes as a set, as a synth would; the counter version is still used for the
well-formed mixer sequences, where it is exactly what catches a double-offset gate.

### Known gaps left deliberately

- **Timing is still `f64` tick accumulation.** Integer timing is §A2.5 and was left alone so this
  step stayed a refactor rather than a behaviour change. Writing the tests did expose the fragility
  first-hand: when a sequence swap lands the play head exactly on an event's tick, whether that event
  fires depends on float rounding. `test_loading_a_longer_sequence_does_not_burst` deliberately lands
  mid-step to avoid it. That is more evidence for the integer rewrite, not a new problem.
- **`run()` itself is still untested** — infinite loop plus `DeviceState::new()`, which needs a
  display server. Everything it calls is covered; removing `device_query` is §A3.
- The mid-bar sequence swap and its `current_tick` rescaling are unchanged. §C2 replaces both with a
  bar-boundary swap; the tests pin current behaviour so that change is visible when it happens.

---

## Changelog — roadmap step 1 (implemented)

### Correctness (§A1)

- **Double gate offset (A1.1).** The Note-Off tick offset was applied at the call site *and* inside
  `add_note_off`. Gate length now comes from `NoteDuration::steps()` and is applied once, in
  `push_note`, and clamped inside `total_ticks` so a release can never land past the loop point.
- **Unsorted events (A1.2).** `PolyphonicSequence`'s fields are now private and
  `PolyphonicSequence::new` is the only constructor; it sorts by `(tick, NoteOff-before-NoteOn)`.
  Accessors are `events()`, `total_ticks()`, `is_empty()`.
- **`num::abs_sub` (A1.3).** Replaced with an equal-power crossfade: `0.0` is the left sequencer
  alone, `1.0` the right, `0.5` both. Velocity now varies continuously with the ratio instead of
  gating a boolean, and voices that fade below `MIN_AUDIBLE_VELOCITY` are dropped before the random
  humanize jitter is applied, so a crossfaded-out voice cannot be nudged back into audibility.
  Coincident identical pitches collapse to a single note instead of two stacked Note-Ons.
- **Silent startup (A1.4).** `EuclideanSequencer::run` emits once before entering its change-detect
  loop, and the default `pulses` went from 0 to 4.
- **A1.5 list.** Empty-MIDI-port-list now errors with a usable message instead of indexing `[0]`;
  the Ctrl-C handler is installed before `Gui::run` takes the thread; `PlaybackHandler::run` no
  longer drops its own `Result` (per-message errors are logged, `?` no longer kills the loop);
  `decrease_phase` wraps symmetrically; `change_pitch` widens to `i16` before clamping;
  `midi_to_note_name` no longer underflows below C0; `SetBPM`/`SetMidiChannel` are actually sent
  (with `=`/`-` bound to tempo) and the engine's duplicate channel counter is gone; dead
  `clock_ticks`/`quarter_notes`/`MixedSequence` removed.
- **Beyond the audit:** pausing used to leave held notes sounding indefinitely. The engine now
  tracks sounding notes in a `[u16; 128]` channel bitmap and releases them explicitly on pause,
  channel change, output-connection change and sequence load — CC 123 is kept only as a backstop.
  Pitch and velocity are masked to 0..=127 so a bad value cannot corrupt the MIDI stream.

### Tests

1 → 21. The mixer regressions were each verified to fail when the original bug is reintroduced:
reinstating the double offset fails `test_gate_ends_before_the_next_step` and
`test_every_event_falls_inside_the_sequence`; reinstating the `abs_sub` gate fails
`test_no_ratio_produces_silence` and `test_ratio_extremes_isolate_one_sequencer`.

### Manifest and tooling (§A5, §A6)

- Dropped `markov`, `throttle`, `rustc-hash`, `ctrlc` (all unreferenced); `tokio`'s redundant
  `sync`/`time` features folded into `full`; `midir` 0.10 → 0.11 (its `find_port_by_id` now takes
  `&str`).
- `rust-version = "1.87"`, **verified** by checking the whole tree with a real 1.87.0 toolchain.
- `[profile.release]` with `lto = "thin"`, `codegen-units = 1`, `panic = "abort"` — verified to
  build.
- `rustfmt.toml` reduced to stable-only options; it emitted ~30 nightly warnings per run before.
  The width heuristics are kept explicit so the existing formatting is preserved.
- `.gitignore` no longer excludes `Cargo.lock` (binary crate) or `flake.lock` (which was tracked
  anyway). **`Cargo.lock` still needs to be committed** — CI uses `--locked`.
- `.github/workflows/ci.yml`: fmt, `clippy --all-targets -- -D warnings`, tests, plus a job pinned
  to the declared MSRV.
- Dockerfile: fixed the broken `COPY src/entrypoint.sh` (the file is at the repo root), removed the
  no-op `RUN #cargo build --release` in favour of an explicit comment, added the native deps the
  build actually needs, and **removed sshd along with the committed root password**. That password
  is in git history and should be considered burned. `entrypoint.sh` no longer restarts a service
  that isn't installed. Obsolete `version:` keys dropped from the three compose files.

### Known gaps left deliberately

- `init.sh` still invokes `ttymidi`, which the image does not install — the arm64 container starts
  but the MIDI bridge does not. Fixing it means either building ttymidi in the image or taking the
  `serialport` route in §A6.
- `ttymidi.tar.gz` is still vendored. It was re-added deliberately (`857cff2 "ttymidi is back
  baby"`) because upstream may disappear, so it was left alone.
- `PlaybackHandler::run` got a 10 ms idle sleep so it stops pegging a core, but it is still a
  polling loop. The real fix is §A2.
- Euclidean generation is the Bresenham approximation, not canonical Euclidean rhythm — E(3,8)
  comes out as `x.x..x..` rather than `x..x..x.`. Tests pin current behaviour; changing it is a
  musical decision, not a bug fix.
- `pulses` is not clamped to `steps`, so pulses > steps still produces a degenerate pattern.
