fn main() {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    if env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default() == "riscv64" {
        let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
        let ld = out_dir.join("linker.ld");
        fs::write(&ld, LINKER_SCRIPT).unwrap();
        println!("cargo:rustc-link-arg=-T{}", ld.display());

        let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let disk_path = manifest_dir.join("disk.img");
        ensure_disk_image(&disk_path, 8 * 1024 * 1024);
    }

    println!("cargo:rerun-if-changed=build.rs");
}

fn ensure_disk_image(path: &std::path::Path, size: u64) {
    use std::fs::OpenOptions;

    if path.exists() {
        return;
    }

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .unwrap();
    file.set_len(size).unwrap();
}

const LINKER_SCRIPT: &[u8] = b"
OUTPUT_ARCH(riscv)
ENTRY(_m_start)

M_BASE_ADDRESS = 0x80000000;
S_BASE_ADDRESS = 0x80200000;

SECTIONS {
    . = M_BASE_ADDRESS;
    .text.m_entry : { *(.text.m_entry) }
    .text.m_trap  : { *(.text.m_trap)  }
    .bss.m_stack  : { *(.bss.m_stack)  }
    .bss.m_data   : { *(.bss.m_data)   }

    . = S_BASE_ADDRESS;
    .text   : {
        *(.text.entry)
        *(.text .text.*)
    }
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    .data   : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    .bss    : {
        *(.bss.uninit)
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }
}";
