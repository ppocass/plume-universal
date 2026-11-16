# Plume

Plume is a local dictation app for Linux and Windows. You talk, the text is
typed into whatever window has the focus, and the audio never leaves the
computer: speech recognition runs on the CPU, offline.

The engine is **NVIDIA Parakeet TDT 0.6B v3** (int8, 25 European languages) via
[sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) 2.0.1. Whisper small and
base are also available in the app for other languages.

Nothing leaves the computer, with one exception you control: the optional
"cleanup" (punctuation, accents, spelling) sends the transcribed **text**, not
the audio, to the OpenAI-compatible endpoint you configure in Settings. Leave it
off and Plume is fully offline.

## Install (Linux Mint 22 / Ubuntu 24.04)

The package is `plume_0.1.0_amd64.deb` (about 18 MB download, 56 MB installed,
plus the speech model downloaded on first launch, about 640 MB).

1. Download `plume_0.1.0_amd64.deb` from the releases page.
2. **Double-click the file.** The Software Manager (or GDebi) opens: click
   *Install*, type your password, done. The Plume icon then appears in the
   application menu.
3. Or, in a terminal:

   ```sh
   sudo apt install ./plume_0.1.0_amd64.deb
   ```

Nothing is published yet: the release workflow prepares a **draft** release on
GitHub. Until it is published, build the .deb as described in
`build/README.md`.

Ubuntu 24.04 and Linux Mint 22 are the minimum: the binary needs glibc 2.39.
Ubuntu 22.04 (glibc 2.35) can install the package but the loader warns about
`GLIBC_2.39`; it is **not supported**.

The release workflow also builds the .deb on Ubuntu 22.04
(`plume_0.1.0_amd64_ubuntu-22.04.deb`) for older systems. That build has not been
tested here.

### First launch

The first start opens the setup assistant, in eight steps: welcome, microphone,
text insertion, shortcut, language, model, cleanup, ready. It proposes:

- **Microphone**: the default input device, with a level test.
- **Text insertion**: on X11, Plume pastes with `xdotool` (installed as a
  dependency of the .deb).
- **Shortcut**: **Left Alt** by default, changeable (Right Alt, Right Ctrl,
  Right Super, F1-F24).
- **Language**: automatic, or one of the 25 Parakeet languages.
- **Model**: Parakeet TDT 0.6B v3, downloaded on first use (about 640 MB).
  Whisper small and base are offered for other languages.
- **Cleanup**: optional, off by default; needs an endpoint and a key.
- **Ready**.

After that, Plume lives in the notification area (tray). Closing the window does
not quit it: *Quit Plume* in the tray menu does.

Plume starts at login by default (an autostart entry is written to
`~/.config/autostart/plume.desktop`). The switch is in Settings.

## The gestures

- **Tap Left Alt** to start dictation; tap it again to stop. The text is then
  pasted into the active window. A tap is a press and release of the key alone,
  under 0.8 s: Alt+letter and Alt+click still work normally.
- **Double-tap Left Alt** while recording to cancel: nothing is transcribed and
  nothing is pasted.
- **Hold Super when you tap to stop** to get the dictation in 1337 (leetspeak).
  The key is configurable (1337 page).
- The pill at the top or bottom of the screen shows the state: listening,
  thinking, pasted, or cancelled.

The history keeps every dictation (text and date), with search, copy and delete.
The tray menu has *Start Dictation*, *Copy Last Dictation*, *History*,
*Settings*, *Support Plume* and *Quit Plume*.

## X11 and Wayland

Plume inserts text by pasting, which needs a global shortcut and a way to send
Ctrl+V to the focused window:

- **X11** (the default session of Linux Mint 22): works. `xdotool` is installed
  with the .deb.
- **Wayland**: not supported. Reading the global shortcut and pasting into
  another application are restricted by the compositor; the app looks for
  `ydotool` but it is not packaged. Use an X11 session.

## Speed

Measured on an Intel Core i7-6700 (4 cores / 8 threads, no GPU), with
sherpa-onnx 2.0.1 and the Parakeet int8 model, 4 threads:

| Audio | Inference | Real-time factor |
|---|---:|---:|
| 30.6 s of French speech | **2.3 s** | 0.074 |

Model loading takes about 2.5 s, once per launch. Parakeet never exceeds
RTF 0.175 on the test files, while Whisper large-v3-turbo takes 45 s on the same
30.6 s of audio. Full numbers and raw logs: `docs/bench-zcomputer.md`.

Silence, room noise and a mouse click produce an empty string (no hallucinated
sentence), which is why Plume does not paste anything in those cases.

## What is not tested yet

- A **real microphone on a physical machine**. Everything was tested in
  containers (model, engine, packaging, X11 under Xvfb), but not with a person
  speaking into a real microphone on a real desktop.
- **Wayland**.
- **Windows**. The code, the workflow and the installer configuration are there,
  but no Windows build has ever been run. Treat the first Windows build as
  experimental.

## Uninstall

```sh
sudo apt remove plume
```

The settings, history and downloaded models stay:

```sh
rm -rf ~/.config/plume ~/.local/share/plume
rm -f ~/.config/autostart/plume.desktop
```

## Licence

Plume is MIT. Third-party components:

- **sherpa-onnx** 2.0.1, Apache-2.0 (<https://github.com/k2-fsa/sherpa-onnx>),
  including onnxruntime (MIT).
- **Parakeet TDT 0.6B v3**, NVIDIA, **CC-BY-4.0** — the model card is
  <https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3> ("License: cc-by-4.0").
  The model is downloaded by the app, not shipped in the package.
- **Inter** (UI font), SIL Open Font License 1.1.
- **Lucide** (icons), ISC.

The full texts of the font and icon licences are in
`crates/plume-app/ui/fonts/LICENSE.txt` and `crates/plume-app/ui/icons/LICENSE`.
