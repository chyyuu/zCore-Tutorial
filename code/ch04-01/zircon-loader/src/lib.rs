#![no_std]
#![feature(asm)]
#![deny(warnings, unused_must_use)]

#[macro_use]
extern crate alloc;
#[macro_use]
extern crate log;

use {
    alloc::{boxed::Box, sync::Arc, vec::Vec},
    core::{future::Future, pin::Pin},
    kernel_hal::MMUFlags,
    xmas_elf::ElfFile,
    zircon_object::{dev::*, ipc::*, object::*, task::*, util::elf_loader::*, vm::*},
};

mod kcounter;

// These describe userboot itself
const K_PROC_SELF: usize = 0;
const K_VMARROOT_SELF: usize = 1;
// Essential job and resource handles
const K_ROOTJOB: usize = 2;
const K_ROOTRESOURCE: usize = 3;
// Essential VMO handles
const K_ZBI: usize = 4;
const K_FIRSTVDSO: usize = 5;
const K_CRASHLOG: usize = 8;
const K_COUNTERNAMES: usize = 9;
const K_COUNTERS: usize = 10;
const K_FISTINSTRUMENTATIONDATA: usize = 11;
const K_HANDLECOUNT: usize = 15;

/// Program images to run.
pub struct Images<T: AsRef<[u8]>> {
    pub userboot: T,
    pub vdso: T,
    pub zbi: T,
}

pub fn run_userboot(images: &Images<impl AsRef<[u8]>>, cmdline: &str) -> Arc<Process> {
    let job = Job::root();
    let proc = Process::create(&job, "userboot").unwrap();
    let thread = Thread::create(&proc, "userboot").unwrap();
    let resource = Resource::create(
        "root",
        ResourceKind::ROOT,
        0,
        0x1_0000_0000,
        ResourceFlags::empty(),
    );
    let vmar = proc.vmar();

    // userboot
    let (entry, userboot_size) = {
        let elf = ElfFile::new(images.userboot.as_ref()).unwrap();
        let size = elf.load_segment_size();
        let vmar = vmar
            .allocate(None, size, VmarFlags::CAN_MAP_RXW, PAGE_SIZE)
            .unwrap();
        vmar.load_from_elf(&elf).unwrap();
        (vmar.addr() + elf.header.pt2.entry_point() as usize, size)
    };

    // vdso
    let vdso_vmo = {
        let elf = ElfFile::new(images.vdso.as_ref()).unwrap();
        let vdso_vmo = VmObject::new_paged(images.vdso.as_ref().len() / PAGE_SIZE + 1);
        vdso_vmo.write(0, images.vdso.as_ref()).unwrap();
        let size = elf.load_segment_size();
        let vmar = vmar
            .allocate_at(
                userboot_size,
                size,
                VmarFlags::CAN_MAP_RXW | VmarFlags::SPECIFIC,
                PAGE_SIZE,
            )
            .unwrap();
        vmar.map_from_elf(&elf, vdso_vmo.clone()).unwrap();
        let offset = elf
            .get_symbol_address("zcore_syscall_entry")
            .expect("failed to locate syscall entry") as usize;
        let syscall_entry = &(kernel_hal_unix::syscall_entry as usize).to_ne_bytes();
        // fill syscall entry x3
        vdso_vmo.write(offset, syscall_entry).unwrap();
        vdso_vmo.write(offset + 8, syscall_entry).unwrap();
        vdso_vmo.write(offset + 16, syscall_entry).unwrap();
        vdso_vmo
    };

    // zbi
    let zbi_vmo = {
        let vmo = VmObject::new_paged(images.zbi.as_ref().len() / PAGE_SIZE + 1);
        vmo.write(0, images.zbi.as_ref()).unwrap();
        vmo.set_name("zbi");
        vmo
    };

    // stack
    const STACK_PAGES: usize = 8;
    let stack_vmo = VmObject::new_paged(STACK_PAGES);
    let flags = MMUFlags::READ | MMUFlags::WRITE | MMUFlags::USER;
    let stack_bottom = vmar
        .map(None, stack_vmo.clone(), 0, stack_vmo.len(), flags)
        .unwrap();
    // WARN: align stack to 16B, then emulate a 'call' (push rip)
    let sp = stack_bottom + stack_vmo.len() - 8;

    // channel
    let (user_channel, kernel_channel) = Channel::create();
    let handle = Handle::new(user_channel, Rights::DEFAULT_CHANNEL);

    let mut handles = vec![Handle::new(proc.clone(), Rights::empty()); K_HANDLECOUNT];
    handles[K_PROC_SELF] = Handle::new(proc.clone(), Rights::DEFAULT_PROCESS);
    handles[K_VMARROOT_SELF] = Handle::new(proc.vmar(), Rights::DEFAULT_VMAR | Rights::IO);
    handles[K_ROOTJOB] = Handle::new(job, Rights::DEFAULT_JOB);
    handles[K_ROOTRESOURCE] = Handle::new(resource, Rights::DEFAULT_RESOURCE);
    handles[K_ZBI] = Handle::new(zbi_vmo, Rights::DEFAULT_VMO);
    // set up handles[K_FIRSTVDSO..K_LASTVDSO + 1]
    const VDSO_DATA_CONSTANTS: usize = 0x4a50;
    const VDSO_DATA_CONSTANTS_SIZE: usize = 0x78;
    let constants: [u8; VDSO_DATA_CONSTANTS_SIZE] =
        unsafe { core::mem::transmute(kernel_hal::vdso_constants()) };
    vdso_vmo.write(VDSO_DATA_CONSTANTS, &constants).unwrap();
    vdso_vmo.set_name("vdso/full");
    let vdso_test1 = vdso_vmo.create_child(false, 0, vdso_vmo.len()).unwrap();
    vdso_test1.set_name("vdso/test1");
    let vdso_test2 = vdso_vmo.create_child(false, 0, vdso_vmo.len()).unwrap();
    vdso_test2.set_name("vdso/test2");
    handles[K_FIRSTVDSO] = Handle::new(vdso_vmo, Rights::DEFAULT_VMO | Rights::EXECUTE);
    handles[K_FIRSTVDSO + 1] = Handle::new(vdso_test1, Rights::DEFAULT_VMO | Rights::EXECUTE);
    handles[K_FIRSTVDSO + 2] = Handle::new(vdso_test2, Rights::DEFAULT_VMO | Rights::EXECUTE);
    // TODO: use correct CrashLogVmo handle
    let crash_log_vmo = VmObject::new_paged(1);
    crash_log_vmo.set_name("crashlog");
    handles[K_CRASHLOG] = Handle::new(crash_log_vmo, Rights::DEFAULT_VMO);
    let (counter_name_vmo, kcounters_vmo) = kcounter::create_kcounter_vmo();
    handles[K_COUNTERNAMES] = Handle::new(counter_name_vmo, Rights::DEFAULT_VMO);
    handles[K_COUNTERS] = Handle::new(kcounters_vmo, Rights::DEFAULT_VMO);
    // TODO: use correct Instrumentation data handle
    let instrumentation_data_vmo = VmObject::new_paged(0);
    instrumentation_data_vmo.set_name("UNIMPLEMENTED_VMO");
    handles[K_FISTINSTRUMENTATIONDATA] =
        Handle::new(instrumentation_data_vmo.clone(), Rights::DEFAULT_VMO);
    handles[K_FISTINSTRUMENTATIONDATA + 1] =
        Handle::new(instrumentation_data_vmo.clone(), Rights::DEFAULT_VMO);
    handles[K_FISTINSTRUMENTATIONDATA + 2] =
        Handle::new(instrumentation_data_vmo.clone(), Rights::DEFAULT_VMO);
    handles[K_FISTINSTRUMENTATIONDATA + 3] =
        Handle::new(instrumentation_data_vmo, Rights::DEFAULT_VMO);

    // check: handle to root proc should be only
    let data = Vec::from(cmdline.replace(':', "\0") + "\0");
    let msg = MessagePacket { data, handles };
    kernel_channel.write(msg).unwrap();

    proc.start(&thread, entry, sp, Some(handle), 0, thread_fn)
        .expect("failed to start main thread");
    proc
}

async fn new_thread(thread: CurrentThread) {
    kernel_hal::Thread::set_tid(thread.id(), thread.proc().id());
    let mut cx = thread.wait_for_run().await;
    trace!("go to user: {:#x?}", cx);
    debug!("switch to {}|{}", thread.proc().name(), thread.name());
    kernel_hal::context_run(&mut cx);
    panic!("OK! back from user: {:#x?}", cx);
}

fn thread_fn(thread: CurrentThread) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
    Box::pin(new_thread(thread))
}
