//
//
//

module_define!{arch, [], init}

pub mod memory;

pub mod sync;

pub mod interrupts;

pub mod boot;

pub mod pci;

pub mod threads;

mod fdt;
mod fdt_devices;

mod aeabi_unwind;

#[inline(always)]
pub fn checkmark() {
	// SAFE: nop ASM
	unsafe { asm!("mov r1, r1" : : : "memory" : "volatile"); }
}
#[inline(always)]
pub fn checkmark_val<T>(v: *const T) {
	// SAFE: nop ASM
	unsafe { asm!("mov r1, r1; mov $0,$0" : : "r"(v) : "memory" : "volatile"); }
}

#[allow(improper_ctypes)]
extern "C" {
	pub fn drop_to_user(entry: usize, stack: usize, args_len: usize) -> !;
}

fn init()
{
	interrupts::init();
}

#[no_mangle]
pub unsafe extern fn hexdump(base: *const u8, size: usize) {
	puts("hexdump("); puth(base as usize as u64); puts(", "); puth(size as u64); puts("): ");
	for i in 0 .. size {
		let v = *base.offset(i as isize);
		put_nibble(v/16);
		put_nibble(v%16);
		putb(b' ');
	}
	putb(b'\n');
}

fn put_nibble(n: u8) {
	if n < 10 {
		putb( b'0' + n );
	}
	else {
		putb( b'a' + n - 10 );
	}
}

fn putb(b: u8) {
	// SAFE: Access should be correct, and no race is possible
	unsafe {
		// - First HWMap page is the UART
		let uart = 0xF100_0000 as *mut u8;
		::core::intrinsics::volatile_store( uart.offset(0), b );
	}
}
#[inline(never)]
#[no_mangle]
pub fn puts(s: &str) {
	//putb(b'(');
	//puth(s.as_ptr() as usize as u64);
	//putb(b',');
	//puth(s.len() as usize as u64);
	//putb(b')');
	for b in s.bytes() {
		putb(b);
	}
}
#[inline(never)]
#[no_mangle]
pub fn puth(v: u64) {
	putb(b'0');
	putb(b'x');
	if v == 0 {
		putb(b'0');
	}
	else {
		for i in (0 .. 16).rev() {
			if v >> (i * 4) > 0 {
				let n = ((v >> (i * 4)) & 0xF) as u8;
				if n < 10 {
					putb( b'0' + n );
				}
				else {
					putb( b'a' + n - 10 );
				}
			}
		}
	}
}

pub fn cur_timestamp() -> u64 {
	0
}

pub fn print_backtrace() {
	let rs = aeabi_unwind::UnwindState::new_cur();
	let addr = rs.get_lr() as usize;
	print_backtrace_unwindstate(rs, addr);
}
fn print_backtrace_unwindstate(mut rs: aeabi_unwind::UnwindState, mut addr: usize)
{
	while let Some(info) = aeabi_unwind::get_unwind_info_for(addr)
	{
		//log_debug!("addr={:#x} fcn={:#x}, info={:#x}", addr, info.0, info.1);
		// - Subtract 1 to avoid 'bl' at the end of a function tricking the resolution
		match ::symbols::get_symbol_for_addr(addr-1)
		{
		Some( (name,ofs) ) => log_debug!("> {:#x} {}+{:#x}", addr, ::symbols::Demangle(name), ofs+1),
		None => log_debug!("> {:#x}", addr),
		}
		match rs.unwind_step(info.1)
		{
		Ok(_) => {},
		Err(e) => {
			log_debug!("- Error {:?}", e);
			return;
			},
		}
		if addr == rs.get_lr() as usize {
			log_warning!("- Same stack frame detected {:#x}", addr);
			break;
		}
		addr = rs.get_lr() as usize;
	}
	log_debug!("- LR={:#x}", rs.get_lr());
}


#[repr(C)]
pub struct AbortRegs
{
	sp: u32,
	lr: u32,
	gprs: [u32; 13],	// R0-R12
	_unused: u32,	// Padding (actually R0)
	ret_pc: u32,	// SRSFD/RFEFD state
	spsr: u32,
}
#[no_mangle]
pub fn data_abort_handler(pc: u32, reg_state: &AbortRegs, dfar: u32, dfsr: u32) {

	log_warning!("Data abort by {:#x} address {:#x} status {:#x}", pc, dfar, dfsr);
	//log_debug!("Registers:");
	//log_debug!("R 0 {:08x}  R 1 {:08x}  R 2 {:08x}  R 3 {:08x}  R 4 {:08x}  R 5 {:08x}}  R 6 {:08x}", reg_state.gprs[0]);
	
	let rs = aeabi_unwind::UnwindState::from_regs([
		reg_state.gprs[0], reg_state.gprs[1], reg_state.gprs[ 2], reg_state.gprs[3],
		reg_state.gprs[4], reg_state.gprs[5], reg_state.gprs[ 6], reg_state.gprs[7],
		reg_state.gprs[8], reg_state.gprs[9], reg_state.gprs[10], reg_state.gprs[11],
		reg_state.gprs[12], reg_state.sp, reg_state.lr, reg_state.ret_pc,
		]);
	print_backtrace_unwindstate(rs, pc as usize);
}
fn fsr_name(ifsr: u32) -> &'static str {
	match ifsr & 0x40F
	{
	0x001 => "Alignment fault",
	0x004 => "Instruction cache maintainence",
	0x00C => "Sync Ext abort walk lvl1",
	0x00E => "Sync Ext abort walk lvl2",
	0x40C => "Sync Ext pairity walk lvl1",
	0x40E => "Sync Ext pairity walk lvl2",
	0x005 => "Translation fault lvl1",
	0x007 => "Translation fault lvl2",
	0x003 => "Access flag fault lvl1",
	0x006 => "Access flag fault lvl2",
	0x009 => "Domain fault lvl1",
	0x00B => "Domain fault lvl2",
	0x00D => "Permissions fault lvl1",
	0x00F => "Permissions fault lvl2",
	_ => "undefined",
	}
}
#[no_mangle]
pub fn prefetch_abort_handler(pc: u32, reg_state: &AbortRegs, ifsr: u32) {
	log_warning!("Prefetch abort at {:#x} status {:#x} ({}) - LR={:#x}", pc, ifsr, fsr_name(ifsr), reg_state.lr);
	//log_debug!("Registers:");
	//log_debug!("R 0 {:08x}  R 1 {:08x}  R 2 {:08x}  R 3 {:08x}  R 4 {:08x}  R 5 {:08x}}  R 6 {:08x}", reg_state.gprs[0]);
	
	let rs = aeabi_unwind::UnwindState::from_regs([
		reg_state.gprs[0], reg_state.gprs[1], reg_state.gprs[ 2], reg_state.gprs[3],
		reg_state.gprs[4], reg_state.gprs[5], reg_state.gprs[ 6], reg_state.gprs[7],
		reg_state.gprs[8], reg_state.gprs[9], reg_state.gprs[10], reg_state.gprs[11],
		reg_state.gprs[12], reg_state.sp, reg_state.lr, reg_state.ret_pc,
		]);
	print_backtrace_unwindstate(rs, pc as usize);
}

pub mod x86_io {
	pub unsafe fn inb(_p: u16) -> u8 { panic!("calling inb on ARM") }
	pub unsafe fn inw(_p: u16) -> u16 { panic!("calling inw on ARM") }
	pub unsafe fn inl(_p: u16) -> u32 { panic!("calling inl on ARM") }
	pub unsafe fn outb(_p: u16, _v: u8) {}
	pub unsafe fn outw(_p: u16, _v: u16) {}
	pub unsafe fn outl(_p: u16, _v: u32) {}
}



#[allow(private_no_mangle_fns)]
#[allow(dead_code)]
mod helpers
{
	#[repr(C)]
	pub struct ulldiv_t { quo: u64, rem: u64, }
	#[no_mangle]
	#[linkage="external"]
	extern fn __aeabi_uldivmod_(n: u64, d: u64, rv: &mut ulldiv_t) {
		*rv = __aeabi_uldivmod(n, d);
	}
	fn __aeabi_uldivmod(mut n: u64, mut d: u64) -> ulldiv_t {
		let mut ret = 0;
		let mut add = 1;
		while n / 2 >= d && add != 0 { d <<= 1; add <<= 1; }
		while add > 0 { if n >= d { ret += add; n -= d; } add  >>= 1; d >>= 1; }
	
		ulldiv_t { quo: ret, rem: n, }
	}
	#[no_mangle]
	#[linkage="external"]
	extern fn __umoddi3(n: u64, d: u64) -> u64 {
		__aeabi_uldivmod(n, d).rem
	}
	
	#[repr(C)]
	pub struct lldiv_t { quo: i64, rem: i64, }
	#[no_mangle]
	#[linkage="external"]
	extern fn __aeabi_ldivmod(n: i64, d: i64) -> lldiv_t {
		let sign = (n < 0) != (d < 0);
		
		let n = if n > 0 { n as u64 } else if n == -0x80000000_00000000 { 1 << 63 } else { -n as u64 };
		let d = if d > 0 { d as u64 } else if d == -0x80000000_00000000 { 1 << 63 } else { -d as u64 };
		let r = __aeabi_uldivmod(n, d);
		if sign {
			lldiv_t {
				quo: -(r.quo as i64),
				rem: -(r.rem as i64),
			}
		}
		else {
			lldiv_t {
				quo: r.quo as i64,
				rem: r.rem as i64,
			}
		}
	}
	#[no_mangle]
	pub extern fn __moddi3(n: i64, d: i64) -> i64 {
		__aeabi_ldivmod(n, d).rem
	}
	
	#[repr(C)]
	pub struct uidiv_t {
		quo: u32,
		rem: u32,
	}
	#[no_mangle]
	#[linkage="external"]
	pub extern fn __aeabi_uidivmod(mut n: u32, mut d: u32) -> uidiv_t {
		let mut ret = 0;
		let mut add = 1;
		while n / 2 >= d && add != 0 { d <<= 1; add <<= 1; }
		while add > 0 { if n >= d { ret += add; n -= d; } add  >>= 1; d >>= 1; }
	
		uidiv_t { quo: ret, rem: n, }
	}
	
	#[no_mangle]
	#[linkage="external"]
	pub extern fn __aeabi_uidiv(n: u32, d: u32) -> u32 {
		__aeabi_uidivmod(n, d).quo
	}
	#[no_mangle]
	#[linkage="external"]
	pub extern fn __umodsi3(n: u32, d: u32) -> u32 {
		__aeabi_uidivmod(n, d).rem
	}
	
	#[repr(C)]
	pub struct idiv_t {
		quo: i32,
		rem: i32,
	}
	#[no_mangle]
	#[linkage="external"]
	pub extern fn __aeabi_idivmod(n: i32, d: i32) -> idiv_t {
		let sign = (n < 0) != (d < 0);
		
		let n = if n > 0 { n as u32 } else if n == -0x80000000 { 1 << 31 } else { -n as u32 };
		let d = if d > 0 { d as u32 } else if d == -0x80000000 { 1 << 31 } else { -d as u32 };
		let r = __aeabi_uidivmod(n, d);
		if sign {
			idiv_t {
				quo: -(r.quo as i32),
				rem: -(r.rem as i32),
			}
		}
		else {
			idiv_t {
				quo: r.quo as i32,
				rem: r.rem as i32,
			}
		}
	}
	#[no_mangle]
	#[linkage="external"]
	extern fn __aeabi_idiv(n: i32, d: i32) -> i32 {
		__aeabi_idivmod(n, d).quo
	}
	#[no_mangle]
	#[linkage="external"]
	extern fn __modsi3(n: i32, d: i32) -> i32 {
		__aeabi_idivmod(n, d).rem
	}
	
	
	#[no_mangle]
	#[linkage="external"]
	extern fn __mulodi4(_a: i32, _b: i32, _of: &mut i32) -> i32 {
		panic!("");
	}
}

