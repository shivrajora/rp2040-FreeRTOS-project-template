# Project template for rp2040-hal with FreeRTOS SMP

This template is a starting point for developing firmware based on the rp2040-hal with **FreeRTOS** as the real-time operating system, running on **both Cortex-M0+ cores** of the RP2040.

It uses my [fork](https://github.com/shivrajora/FreeRTOS-rust) of the [`freertos-rust`](https://crates.io/crates/freertos-rust) crate to integrate the FreeRTOS kernel into a Rust project, compiling the FreeRTOS C source via a build script. The fork adds SMP support and core affinity APIs for the RP2040.

The example `main.rs` demonstrates a blinky application running inside a FreeRTOS task using `Task::new()` and `FreeRtosUtils::start_scheduler()`. An optional **SMP demo** mode (enabled via the `smp-demo` feature flag) creates two tasks pinned to separate cores to showcase dual-core scheduling — see the [Feature flags](#feature-flags) section.

It includes all of the `knurling-rs` tooling as showcased in https://github.com/knurling-rs/app-template (`defmt`, `defmt-rtt`, `panic-probe`, `flip-link`) to make development as easy as possible.

`probe-rs` is configured as the default runner, so you can start your program as easily as
```sh
cargo run --release
```

If you aren't using a debugger (or want to use other debugging configurations), check out [alternative runners](#alternative-runners) for other options.

<!-- TABLE OF CONTENTS -->
<details open="open">
  
  <summary><h2 style="display: inline-block">Table of Contents</h2></summary>
  <ol>
    <li><a href="#freertos-integration">FreeRTOS Integration</a></li>
    <li><a href="#smp-dual-core-support">SMP Dual-Core Support</a></li>
    <li><a href="#quickstart">Quickstart</a></li>
    <li><a href="#markdown-header-requirements">Requirements</a></li>
    <li><a href="#installation-of-development-dependencies">Installation of development dependencies</a></li>
    <li><a href="#project-creation">Project Creation</a></li>
    <li><a href="#running">Running</a></li>
    <li><a href="#alternative-runners">Alternative runners</a></li>
    <li><a href="#notes-on-using-rp2040_boot2">Notes on using rp2040_boot2</a></li>
    <li><a href="#feature-flags">Feature flags</a></li>
    <li><a href="#roadmap">Roadmap</a></li>
    <li><a href="#contributing">Contributing</a></li>
    <li><a href="#code-of-conduct">Code of conduct</a></li>
    <li><a href="#license">License</a></li>
    <li><a href="#contact">Contact</a></li>
  </ol>
</details>

<!-- FreeRTOS Integration -->
<details open="open">
  <summary><h2 style="display: inline-block" id="freertos-integration">FreeRTOS Integration</h2></summary>

This template integrates [FreeRTOS](https://www.freertos.org/) into a Rust embedded project for the RP2040 using the [`freertos-rust`](https://github.com/shivrajora/FreeRTOS-rust) crate.

### How it works

- The FreeRTOS C kernel sources are compiled at build time via a `build.rs` script using [`freertos-cargo-build`](https://github.com/shivrajora/FreeRTOS-rust).
- The [`freertos-rust`](https://github.com/shivrajora/FreeRTOS-rust) crate provides safe Rust bindings to the FreeRTOS API.
- `FreeRtosAllocator` is registered as the global allocator, enabling heap allocation backed by FreeRTOS's heap implementation.
- The **RP2040-specific FreeRTOS port** (`ThirdParty/GCC/RP2040`) is used instead of the generic ARM Cortex-M0 port, enabling full SMP support across both cores.
- A minimal **pico-sdk shim** (`src/port/`) provides direct register access implementations of the pico-sdk C functions required by the port — no pico-sdk dependency is needed.

### Creating a FreeRTOS task

Wrap your application logic in a task and start the scheduler:

```rust
use freertos_rust::*;

#[entry]
fn main() -> ! {
    Task::new()
        .name("app")
        .stack_size(1000)
        .start(move |_| {
            app_main();
        })
        .unwrap();
    FreeRtosUtils::start_scheduler();
}
```

Use `CurrentTask::delay(Duration::ms(500))` instead of busy-waiting to yield the CPU to other tasks during delays.

### Dependencies

The FreeRTOS crates are currently sourced from a fork pending upstreaming:

```toml
freertos-rust = { git = "https://github.com/shivrajora/FreeRTOS-rust.git", branch = "master" }
freertos-cargo-build = { git = "https://github.com/shivrajora/FreeRTOS-rust.git", branch = "master" }
```

</details>

<!-- SMP Dual-Core Support -->
<details open="open">
  <summary><h2 style="display: inline-block" id="smp-dual-core-support">SMP Dual-Core Support</h2></summary>

The RP2040 has two Cortex-M0+ cores. This template runs FreeRTOS in **SMP mode** with both cores active, using the RP2040-specific FreeRTOS port.

### What's different from a single-core setup

| | Single-core (ARM_CM0) | This template (RP2040 SMP) |
|---|---|---|
| FreeRTOS port | `GCC/ARM_CM0` | `ThirdParty/GCC/RP2040` |
| Active cores | 1 | 2 |
| Cross-core yield | N/A | SIO FIFO (`vYieldCore`) |
| Hardware spinlocks | N/A | IDs 26 & 27 (SIO) |
| Core affinity | N/A | Per-task bitmask |
| Exception handler names | `SVC_Handler` etc. | `isr_svcall` etc. |

### FreeRTOSConfig.h SMP settings

```c
#define configNUMBER_OF_CORES       2
#define configTICK_CORE             0   // core 0 drives the tick
#define configUSE_CORE_AFFINITY     1
#define configSMP_SPINLOCK_0        26  // hardware spinlock for SMP critical sections
#define configSMP_SPINLOCK_1        27
```

### Pinning tasks to a specific core

Use the `core_affinity` builder method to pin a task to one or both cores (bitmask: `0b01` = core 0, `0b10` = core 1, `0b11` = either):

```rust
// Run only on core 0
Task::new()
    .name("worker")
    .stack_size(1000)
    .core_affinity(0b01)
    .start(move |_| { /* ... */ })
    .unwrap();

// Run only on core 1
Task::new()
    .name("isr-handler")
    .stack_size(512)
    .core_affinity(0b10)
    .start(move |_| { /* ... */ })
    .unwrap();
```

Without a `core_affinity` call, FreeRTOS distributes tasks across both cores automatically.

### pico-sdk shim

The RP2040 port depends on several pico-sdk C functions (`multicore_launch_core1`, `spin_lock_*`, `irq_set_*`, `clock_get_hz`, etc.). Rather than linking against the full pico-sdk, this template provides lightweight shim implementations in `src/port/`:

| File | Purpose |
|------|---------|
| `src/port/pico_shim.h` | SIO register layout, spinlock ops, NVIC helpers, sync interop stubs |
| `src/port/pico_shim_rp2040.c` | FIFO-based core1 launch, RAM vector table, IRQ management, `clock_get_hz` (125 MHz) |
| `src/port/pico.h` | Shadows pico-sdk's `pico.h`; sets `PICO_NO_RAM_VECTOR_TABLE=1` |
| `src/port/hardware/*.h` | Stub headers for `hardware/sync.h`, `hardware/clocks.h`, `hardware/exception.h`, `hardware/irq.h` |
| `src/port/pico/lock_core.h` | Minimal `lock_core_t` struct for sync interop |
| `src/port/pico/multicore.h` | `multicore_*` function declarations |

</details>

<!-- Quickstart -->
<details open="open">
  <summary><h2 style="display: inline-block" id="quickstart">Quickstart</h2></summary>

This quickstart assumes that you've got a [Raspberry Pi
Pico](https://www.raspberrypi.com/products/raspberry-pi-pico/) (the first-generation
version containing the RP2040 MCU) as well as a [Raspberry Pi Debug
Probe](https://www.raspberrypi.com/products/debug-probe/) and will flash the Pico with
[probe-rs](https://probe.rs/).

Note: you don't have to use this setup. It's just the most common and well-supported setup.
See the rest of the README for instructions on setting up different hardware or software.

1. [Connect](https://www.raspberrypi.com/documentation/microcontrollers/debug-probe.html#getting-started)
   your Raspberry Pi Pico and Debug Probe to your development host.

1. Set up `cargo generate`:

   ```
   cargo install cargo-generate
   ```

1. Start your project by copying this template:

   ```
   cargo generate --git https://github.com/shivrajora/rp2040-FreeRTOS-project-template
   ```

1. Install the cross-compilation toolchain:

   ```
   rustup target install thumbv6m-none-eabi
   ```

1. Install stack overflow protection:

   ```
   cargo install flip-link
   ```

1. Install the flashing tools:

   ```
   cargo install --locked probe-rs-tools
   ```

1. Flash the debug build of the blinky app to your Pico:

   ```
   cargo run
   ```

</details>


<!-- Requirements -->
<details open="open">
  <summary><h2 style="display: inline-block" id="requirements">Requirements</h2></summary>
  
- The standard Rust tooling (cargo, rustup) which you can install from https://rustup.rs/

- Toolchain support for the Cortex-M0+ processors in the RP2040 (thumbv6m-none-eabi)

- `arm-none-eabi-gcc` — used by `freertos-cargo-build` to cross-compile the FreeRTOS C sources
  - macOS: `brew install --cask gcc-arm-embedded`
  - Ubuntu: `apt-get install gcc-arm-none-eabi`

- `flip-link` — detects stack overflows on both cores at runtime

- (by default) A [`probe-rs` installation](https://probe.rs/docs/getting-started/installation)
- A [`probe-rs` compatible](https://probe.rs/docs/getting-started/probe-setup) probe

  You can use a second
  [Pico as a CMSIS-DAP debug probe](debug_probes.md#raspberry-pi-pico). Details
  on other supported debug probes can be found in
  [debug_probes.md](debug_probes.md)

</details>

<!-- Installation of development dependencies -->
<details open="open">
  <summary><h2 style="display: inline-block" id="installation-of-development-dependencies">Installation of development dependencies</h2></summary>

```sh
rustup target install thumbv6m-none-eabi
cargo install flip-link
# Installs the probe-rs tools, including probe-rs run, our recommended default runner
cargo install --locked probe-rs-tools
```

Install the ARM cross-compiler for building FreeRTOS C sources:

```sh
# macOS
brew install --cask gcc-arm-embedded

# Ubuntu / Debian
apt-get install gcc-arm-none-eabi
```

If you want to use picotool instead, install a [picotool binary][] for your system.

[picotool binary]: https://github.com/raspberrypi/pico-sdk-tools/releases

If you get the error ``binary `cargo-embed` already exists`` during installation of probe-rs, run `cargo uninstall cargo-embed` to uninstall your older version of cargo-embed before trying again.

</details>

<!-- Creating the project -->
<details open="open">
  <summary><h2 style="display: inline-block" id="project-creation">Creating your project</h2></summary>

### Using `cargo-generate`

```sh
cargo generate --git https://github.com/shivrajora/rp2040-FreeRTOS-project-template
```

Follow the wizard 🪄 and enjoy your new project.

### Downloading as a zip file or using GitHub's template support

Obtain a copy of the code, either by downloading this repository as a zip file or using GitHub's
template feature, then apply the following:
- Remove `debug_probes.md`.
- Remove the `cargo-generate` directory.
- Remove/edit `README.md`.
- If using vscode update `.vscode/launch.json`;
  Else: remove this file.
- Edit `Cargo.toml` & adjust according to your project (especially its name).
- Edit `.cargo/config.toml` to select your favorite runner.

</details>

<!-- Running -->
<details open="open">
  <summary><h2 style="display: inline-block" id="running">Running</h2></summary>
  
For a debug build
```sh
cargo run
```
For a release build
```sh
cargo run --release
```

To run the SMP demo (two tasks, one per core — see [Feature flags](#feature-flags)):
```sh
cargo run --features smp-demo
```

If you do not specify a DEFMT_LOG level, it will be set to `debug`.
That means `println!("")`, `info!("")` and `debug!("")` statements will be printed.
If you wish to override this, you can change it in `.cargo/config.toml` 
```toml
[env]
DEFMT_LOG = "off"
```
You can also set this inline (on Linux/MacOS)  
```sh
DEFMT_LOG=trace cargo run
```

or set the _environment variable_ so that it applies to every `cargo run` call that follows:
#### Linux/MacOS/unix
```sh
export DEFMT_LOG=trace
```

Setting the DEFMT_LOG level for the current session  
for bash
```sh
export DEFMT_LOG=trace
```

#### Windows
Windows users can only override DEFMT_LOG through `config.toml`
or by setting the environment variable as a separate step before calling `cargo run`
- cmd
```cmd
set DEFMT_LOG=trace
```
- powershell
```ps1
$Env:DEFMT_LOG = trace
```

```cmd
cargo run
```

</details>
<!-- ALTERNATIVE RUNNERS -->
<details open="open">
  <summary><h2 style="display: inline-block" id="alternative-runners">Alternative runners</h2></summary>

If you don't have a debug probe or if you want to do interactive debugging you can set up an alternative runner for cargo.  

Some of the options for your `runner` are listed below:

* **`cargo embed`**
  This is basically a more configurable version of `probe-rs run`, our default runner.
  See [the `cargo-embed` tool docs page](https://probe.rs/docs/tools/cargo-embed) for
  more information.
  
  *Step 1* - Install `cargo-embed`. This is part of the [`probe-rs`](https://crates.io/crates/probe-rs) tools:

  ```sh
  cargo install --locked probe-rs-tools
  ```

  *Step 2* - Update settings in [Embed.toml](./Embed.toml)  
  - The defaults are to flash, reset, and start a defmt logging session
  You can find all the settings and their meanings [in the probe-rs repo](https://github.com/probe-rs/probe-rs/blob/c435072d0f101ade6fc3fde4a7899b8b5ef69195/probe-rs-tools/src/bin/probe-rs/cmd/cargo_embed/config/default.toml)

  *Step 3* - Use the command `cargo embed`, which will compile the code, flash the device
  and start running the configuration specified in Embed.toml

  ```sh
  cargo embed --release
  ```

* **probe-rs-debugger**
  *Step 1* - Install Visual Studio Code from https://code.visualstudio.com/

  *Step 2* - Install `probe-rs`
  ```sh
  cargo install --locked probe-rs-tools
  ```

  *Step 3* - Open this project in VSCode

  *Step 4* - Install `debugger for probe-rs` via the VSCode extensions menu (View > Extensions)

  *Step 5* - Launch a debug session by choosing `Run`>`Start Debugging` (or press F5)

* **Loading over USB with Picotool**  
  *Step 1* - Install a [picotool binary][] for your system.

  *Step 2* - Modify `.cargo/config` to change the default runner

  ```toml
  [target.`cfg(all(target-arch = "arm", target_os = "none"))`]
  runner = "picotool load --update --verify --execute -t elf"
  ```

  The all-Arm wildcard `'cfg(all(target_arch = "arm", target_os = "none"))'` is used
  by default in the template files, but may also be replaced by
  `thumbv6m-none-eabi`.

  *Step 3* - Boot your RP2040 into "USB Bootloader mode", typically by rebooting
  whilst holding some kind of "Boot Select" button.

  *Step 4* - Use `cargo run`, which will compile the code and start the
  specified 'runner'. As the 'runner' is picotool, it will use the PICOBOOT
  interface over USB to flash your RP2040.

  ```sh
  cargo run --release
  ```

</details>
<!-- Notes on using rp2040_hal and rp2040_boot2 -->
<details open="open">
  <summary><h2 style="display: inline-block" id="notes-on-using-rp2040_boot2">Notes on using rp2040_boot2</h2></summary>

  The second-stage boot loader must be written to the .boot2 section. That
  is usually handled by the board support package (e.g.`rp-pico`). If you don't use
  one, you should initialize the boot loader manually. This can be done by adding the
  following to the beginning of main.rs:
  ```rust
  use rp2040_boot2;
  #[link_section = ".boot2"]
  #[used]
  pub static BOOT_LOADER: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;
  ```

</details>

<!-- Feature flags -->
<details open="open">
  <summary><h2 style="display: inline-block" id="feature-flags">Feature flags</h2></summary>

### `smp-demo` — dual-core showcase

Enable a two-task example that demonstrates FreeRTOS running on both Cortex-M0+ cores simultaneously:

```sh
cargo run --features smp-demo
```

| Task | Core | What it does |
|------|------|--------------|
| `led-core0` | 0 (pinned) | Blinks the LED every 500 ms |
| `counter-core1` | 1 (pinned) | Logs a counter every 1 s |

Both tasks log their actual core ID via defmt so you can confirm they are running on the expected core:

```
LED task on core0
core1 counter: 0
on!
core1 counter: 1
off!
...
```

The tasks are pinned using the `core_affinity` builder method on `Task::new()`:

```rust
Task::new()
    .name("led-core0")
    .stack_size(1000)
    .core_affinity(1 << 0)   // 0b01 = core 0 only
    .start(move |_| app_main())
    .unwrap();
```

Without `core_affinity`, FreeRTOS distributes tasks across both cores automatically.

---

### rp2040-hal feature flags

There are also several [feature flags in rp2040-hal](https://docs.rs/rp2040-hal/latest/rp2040_hal/#crate-features).
If you want to enable some of them, uncomment the `rp2040-hal` dependency in `Cargo.toml` and add the
desired feature flags there. For example, to enable ROM functions for f64 math using the feature `rom-v2-intrinsics`:
```
rp2040-hal = { version="0.10", features=["rt", "critical-section-impl", "rom-v2-intrinsics"] }
```
</details>

<!-- ROADMAP -->

## Roadmap

NOTE These packages are under active development. As such, it is likely to
remain volatile until a 1.0.0 release.

See the [open issues](https://github.com/shivrajora/rp2040-FreeRTOS-project-template/issues) for a list of
proposed features (and known issues).

## Contributing

Contributions are what make the open source community such an amazing place to be learn, inspire, and create. Any contributions you make are **greatly appreciated**.

The steps are:

1. Fork the Project by clicking the 'Fork' button at the top of the page.
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Make some changes to the code or documentation.
4. Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
5. Push to the Feature Branch (`git push origin feature/AmazingFeature`)
6. Create a [New Pull Request](https://github.com/shivrajora/rp2040-FreeRTOS-project-template/pulls)
7. An admin will review the Pull Request and discuss any changes that may be required.
8. Once everyone is happy, the Pull Request can be merged by an admin, and your work is part of our project!

## Code of Conduct

Contribution to this crate is organized under the terms of the [Rust Code of
Conduct][CoC], and the maintainer of this crate, the [rp-rs team], promises
to intervene to uphold that code of conduct.

[CoC]: CODE_OF_CONDUCT.md
[rp-rs team]: https://github.com/orgs/rp-rs/teams/rp-rs

## License

The contents of this repository are dual-licensed under the _[MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE-2.0)_ License. That means you can chose either the MIT licence or the
Apache-2.0 licence when you re-use this code. See [`LICENSE-MIT`](LICENSE-MIT) or [`LICENSE-APACHE-2.0`](LICENSE-APACHE-2.0) for more
information on each specific licence.

Any submissions to this project (e.g. as Pull Requests) must be made available
under these terms.

## Contact

Raise an issue: [https://github.com/shivrajora/rp2040-FreeRTOS-project-template/issues](https://github.com/shivrajora/rp2040-FreeRTOS-project-template/issues)

