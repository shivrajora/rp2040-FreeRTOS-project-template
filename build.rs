//! This build script copies the `memory.x` file from the crate root into
//! a directory where the linker can always find it at build time.
//! For many projects this is optional, as the linker always searches the
//! project root directory -- wherever `Cargo.toml` is. However, if you
//! are using a workspace or have a more complicated build setup, this
//! build script becomes required. Additionally, by requesting that
//! Cargo re-run the build script whenever `memory.x` is changed,
//! updating `memory.x` ensures a rebuild of the application with the
//! new memory settings.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    let mut b = freertos_cargo_build::Builder::new();

    // Path to FreeRTOS kernel or set ENV "FREERTOS_SRC" instead
    b.freertos("third_party/FreeRTOS");
    b.freertos_config("src"); // Location of `FreeRTOSConfig.h`
    // RP2040 SMP port: ThirdParty/GCC/RP2040 (Cortex-M0+ dual-core, uses SIO FIFO for vYieldCore)
    b.freertos_port("ThirdParty/GCC/RP2040");
    b.heap("heap_4.c"); // Set the heap_?.c allocator to use from
                        // 'FreeRTOS-Kernel/portable/MemMang' (Default: heap_4.c)

    // Inject pico-sdk shim (direct register access, no real pico-sdk needed)
    b.add_build_file("src/port/pico_shim_rp2040.c");
    // Expose stub headers that shadow pico-sdk's pico.h, hardware/*.h, pico/multicore.h
    b.get_cc().include("src/port");
    // RP2040 port stores portmacro.h in include/ — add it explicitly
    b.get_cc()
        .include("third_party/FreeRTOS/portable/ThirdParty/GCC/RP2040/include");
    // Preprocessor flags required by the RP2040 FreeRTOS port
    b.get_cc().define("LIB_PICO_MULTICORE", "1");
    b.get_cc().define("LIB_PICO_SYNC", "0");
    b.get_cc().define("LIB_PICO_TIME", "0");
    b.get_cc()
        .define("configUSE_DYNAMIC_EXCEPTION_HANDLERS", "0");

    b.compile().unwrap_or_else(|e| panic!("{}", e.to_string()));

    // By default, Cargo will re-run a build script whenever
    // any file in the project changes. By specifying `memory.x`
    // here, we ensure the build script is only re-run when
    // `memory.x` is changed.
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=src/FreeRTOSConfig.h");

    // The RP2040 FreeRTOS port with configUSE_DYNAMIC_EXCEPTION_HANDLERS=0
    // renames the CMSIS handlers to pico-sdk isr_* names:
    //   vPortSVCHandler   → isr_svcall
    //   xPortPendSVHandler → isr_pendsv
    //   xPortSysTickHandler → isr_systick
    // cortex-m-rt's linker script uses PROVIDE(SVCall = DefaultHandler) etc.
    // A strong assignment overrides PROVIDE(), wiring the vector-table slots
    // directly to the FreeRTOS naked-asm handlers.
    // -u forces portasm.o out of libfreertos.a so the symbols exist.
    for sym in &["isr_svcall", "isr_pendsv", "isr_systick"] {
        println!("cargo:rustc-link-arg=-u");
        println!("cargo:rustc-link-arg={sym}");
    }
    let vectors_ld = out.join("freertos-vectors.x");
    std::fs::write(
        &vectors_ld,
        b"SVCall  = isr_svcall;\nPendSV  = isr_pendsv;\nSysTick = isr_systick;\n",
    )
    .unwrap();
    println!("cargo:rustc-link-arg=-T{}", vectors_ld.display());
}
