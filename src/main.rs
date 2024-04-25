use std::mem::transmute;
use std::os::raw::c_void;
use std::slice::from_raw_parts_mut;

use windows::Win32::System::Memory::VirtualAlloc;
use windows::Win32::System::Memory::VirtualProtect;
use windows::Win32::System::Memory::MEM_COMMIT;
use windows::Win32::System::Memory::MEM_RESERVE;
use windows::Win32::System::Memory::PAGE_EXECUTE_READ;
use windows::Win32::System::Memory::PAGE_PROTECTION_FLAGS;
use windows::Win32::System::Memory::PAGE_READWRITE;
use windows::Win32::System::Memory::VIRTUAL_ALLOCATION_TYPE;
use windows::Win32::System::SystemInformation::GetSystemInfo;
use windows::Win32::System::SystemInformation::SYSTEM_INFO;

fn main() {
    let program_path = "main.exe";
    let program_image = std::fs::read(program_path).unwrap();
    let program_size = program_image.len();

    unsafe {
        let my_fn = test as *const fn() -> ();
        let my_fn_addr = (my_fn as usize).to_le_bytes();
        let relocation_code = &[
            // Call our routine.
            0x49,
            0xB8,
            my_fn_addr[0],
            my_fn_addr[1],
            my_fn_addr[2],
            my_fn_addr[3],
            my_fn_addr[4],
            my_fn_addr[5],
            my_fn_addr[6],
            my_fn_addr[7],
            0x41,
            0xFF,
            0xD0,
            // Ret back to the original code.
            0xC3,
            // NOTE: R8 register should be reset to the original value.
        ];

        let mem = alloc(None, program_size + relocation_code.len());
        let mem = from_raw_parts_mut(mem, program_size + relocation_code.len());

        let base_program = &mut mem[0..program_size];
        base_program.copy_from_slice(&program_image);

        let relocation = &mut mem[program_size..];
        relocation.copy_from_slice(relocation_code);
        let relocation_addr = relocation.as_ptr();

        let main_offset = 0x430; // Derived from IDA.

        // Change literal value.
        {
            let a_offset = main_offset + 0x9;
            mem[a_offset] = 0x5;
        }

        // Replace function call and jump into our code.
        {
            // Original C program uses near `call` with displacement. Displacement is 32bit value
            // and distance between `text` section of this program and allocated buffer is too
            // big to be represented as 32bit number. Using an absolute call is not possible either
            // because it requires an absolute address to be loaded into one of registers so, the whole
            // thing takes about 12 bytes, while original call is just 5.
            // Thats why we allocate a little bit more space after base program and put relocation
            // code there, it fits within the required bonds and allows us to call into our Rust code.
            // Conrol flow is as follows (Language[Address space section]):
            // Rust[text] -> C[heap] -> CustomASM[heap] -> Rust[text]

            let call_site_offset = 0x460; // Derived from IDA.
            let call_site = &mut mem[call_site_offset..call_site_offset + 5];

            // `call` adds displacement to EPI and EPI points to the next instruction so we need
            // to add 5 because current instruction is 5 bytes.
            let displacement = relocation_addr.sub(call_site.as_ptr() as usize + 5);
            let displacement = (displacement as usize).to_le_bytes();

            call_site.copy_from_slice(&[
                0xE8,
                displacement[0],
                displacement[1],
                displacement[2],
                displacement[3],
            ]);
        }

        make_executable(mem);

        let start = (mem.as_ptr() as *const u8).add(main_offset);
        let func = transmute::<*const u8, fn() -> u32>(start);
        let val = func();
        dbg!(val);
    }
}

fn test() {
    // If the original finction received arguments it should be possible to
    // access them here using `asm!` macro.
    let a = 0;
    dbg!(a);
    let b = 1;
}

unsafe fn make_executable(mem: &mut [u8]) {
    let mut old_flags = PAGE_PROTECTION_FLAGS(0);
    let _ = VirtualProtect(
        mem.as_ptr() as *const _,
        mem.len(),
        PAGE_EXECUTE_READ,
        &mut old_flags,
    );
}

unsafe fn alloc(base_ptr: Option<*const c_void>, size: usize) -> *mut u8 {
    let mem = VirtualAlloc(base_ptr, size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) as *mut u8;
    return mem;
}
