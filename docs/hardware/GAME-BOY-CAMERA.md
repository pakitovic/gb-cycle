# Game Boy Camera

Pocket Camera is cartridge-local hardware. It is not an ordinary `MBC3` / `MBC5` variant and it must not be modeled as a frontend-only hack.

This document refines the Pocket Camera-specific behavior under the broader cartridge rules from [`CARTRIDGES-MBC.md`](./CARTRIDGES-MBC.md).

## Ownership boundary

- `gb-core` owns:
  - the dedicated `0xFC` cartridge family
  - ROM / RAM banking
  - the register window at `0xA000-0xBFFF`
  - capture busy / pause / resume timing in **T-cycles**
  - the image-processing pipeline that writes tiles into cartridge SRAM
  - cartridge-owned persistence for the full `128 KiB` SRAM payload
- frontends own:
  - file dialogs, webcam permissions, camera APIs, and host-specific frame acquisition
  - optional session UX such as choosing or clearing a still image
- The core/frontend seam is an explicit grayscale frame API. The core must never depend on a host webcam API directly.

## Current baseline

- Header code `0xFC` is supported in `Strict`.
- Validation accepts only the official commercial shape:
  - ROM size code `0x05`
  - actual ROM length `1 MiB`
  - RAM size code `0x04`
  - effective SRAM length `128 KiB`
- `gb-core` exposes:
  - `PocketCameraFrame { width, height, grayscale_pixels }`
  - `Machine::has_pocket_camera()`
  - `Machine::set_pocket_camera_frame(...)`
  - `Machine::clear_pocket_camera_frame()`
- `gb-desktop` exposes session-scoped:
  - `CAM IMAGE` / `CAM RESET` still-image flow backed by PNG decoding
  - `CAM LIVE` live-frame flow backed by SDL3 camera capture
- Live camera support remains frontend-only: the desktop frontend opens the host camera with SDL's native stream selection, handles permission/device errors, drops a few warm-up frames, converts each acquired frame through RGB24 into grayscale, and repeatedly calls the same `Machine::set_pocket_camera_frame(...)` seam. The core has no webcam dependency.

## Official ROM validation matrix

Manual acceptance currently covers the known official retail / special-release Pocket Camera software set:

| ROM name | Region / variant | V1 result |
| --- | --- | --- |
| `Game Boy Camera (USA, Europe) (SGB Enhanced)` | international retail | boot, `CAM IMAGE` capture, SRAM photo save / reload, and `CAM RESET` verified |
| `Game Boy Camera Gold (USA) (SGB Enhanced)` | U.S. Nintendo Power / Zelda-themed special release | boot, `CAM IMAGE` capture, SRAM photo save / reload, and `CAM RESET` verified |
| `Pocket Camera (Japan) (Rev 1) (SGB Enhanced)` | Japanese retail revision; also appears in older sets as `Rev A` / `V1.1` | boot, `CAM IMAGE` capture, SRAM photo save / reload, and `CAM RESET` verified |

These checks validate the static-image V1 path against the practical official-ROM matrix. They do not replace future differential/oracle work for analog fidelity or printer-facing edge cases.

Non-retail variants are tracked separately:

- `CoroCoro Comics Pocket Camera` appears to be content re-enabled through save data / cheat state on the Japanese ROM rather than a distinct retail ROM image.
- `Hello Kitty Pocket Camera` is cancelled / prototype-only and is not part of the official retail acceptance set.

## Banking and register model

- `0000-1FFF`: RAM enable. `0x0A` enables SRAM writes. Reads remain available regardless of this bit.
- `2000-3FFF`: switchable ROM bank for `4000-7FFF`. Banks `0x00..=0x3F` are valid, including bank `0`.
- `4000-5FFF`: either:
  - SRAM bank `0x00..=0x0F`, or
  - camera-register window select when `bit4 = 1`
- `A000-BFFF`:
  - SRAM when register-select `bit4 = 0`
  - camera registers when register-select `bit4 = 1`
- Camera registers mirror every `0x80` bytes.

## Register behavior

- `A000`:
  - write `bit0 = 1` starts capture
  - read `bit0` reports busy (`1 = working`, `0 = finished / paused / idle`)
  - bits `1:2` are readable / writable control bits used by the 1-D filtering path
  - writing `bit0 = 0` pauses an in-flight capture
  - writing `bit0 = 1` again resumes that paused capture with the already-latched parameters
- `A001-A005`: sensor configuration registers
- `A006-A035`: `4 x 4 x 3` contrast / dithering matrix
- Only `A000` is meaningfully readable. Other registers read back as `0x00`.

## Busy timing

- Busy timing follows the documented Pocket Camera formula, stored internally as `capture_ready_at: Option<TCycle>`.
- DMG / normal-speed timing:

  `4 * (32446 + (A001.bit7 ? 0 : 512) + 16 * exposure)`

  where `exposure = (A002 << 8) | A003`.

- While capture is working:
  - SRAM reads return `0x00`
  - SRAM writes are ignored
  - `A000` remains readable for busy polling
- If capture is paused:
  - SRAM becomes readable again
  - a later `bit0 = 1` resumes the remaining countdown instead of restarting from scratch
- V1 resolves capture completion lazily on timed cartridge accesses; it does **not** inject a dedicated autonomous scheduler event.

## Image pipeline baseline

- Host frames are grayscale `8-bit` pixels where `0 = black` and `255 = white`.
- The core normalizes any host frame to `128x112` with deterministic nearest-neighbor stretch.
- If the host has not provided a frame, the core uses a deterministic placeholder image:
  - four vertical DMG-style grayscale bars
  - a `1`-pixel border
  - stable contents for tests and reproducible UX
- Capture pipeline baseline:
  - latch the current host frame
  - expand to the sensor-height domain (`128 x 120`) used by the current model
  - apply exposure / invert handling
  - apply the supported filter modes needed by the official ROM flow
  - crop the middle `112` rows
  - apply the `4 x 4 x 3` controller matrix
  - write `14 x 16` tiles into SRAM bank `0` at offset `0x0100`

## Persistence and observability

- Persist only the cartridge-owned `128 KiB` SRAM payload.
- Do **not** persist:
  - busy state
  - volatile registers
  - the currently selected host frame
- Cartridge snapshots should expose:
  - `camera_capture_ready_at`
  - whether the `A000-BFFF` window is currently SRAM or camera registers

## Deferred work

- broader oracle validation for printer-exposed flows and lower-level capture observables
- higher-fidelity analog modeling beyond the current Pan Docs-oriented static-frame baseline
- richer desktop live-camera UX such as device selection, preview, and persistent user preference if that becomes necessary

## References

- [Pan Docs — Game Boy Camera](https://gbdev.io/pandocs/Gameboy_Camera.html)
- [SameBoy `camera.c`](https://github.com/LIJI32/SameBoy/blob/208ba4afabffab9edde416f2dbb8ae459e34adb8/Core/camera.c)
- [mGBA `pocket-cam.c`](https://github.com/mgba-emu/mgba/blob/f3f6589efdecb0b7f878d26444b05d0d7cb69d68/src/gb/mbc/pocket-cam.c)
