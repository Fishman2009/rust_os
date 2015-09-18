
pub mod memory;

pub mod sync;

pub mod interrupts;

pub mod boot;

pub mod pci;

pub mod threads;


pub unsafe fn drop_to_user(entry: usize, stack: usize, args_len: usize) -> !
{
	loop {}
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
}

pub mod x86_io {
	pub unsafe fn inb(_p: u16) -> u8 { panic!("calling inb on ARM") }
	pub unsafe fn inw(_p: u16) -> u16 { panic!("calling inw on ARM") }
	pub unsafe fn inl(_p: u16) -> u32 { panic!("calling inl on ARM") }
	pub unsafe fn outb(_p: u16, _v: u8) {}
	pub unsafe fn outw(_p: u16, _v: u16) {}
	pub unsafe fn outl(_p: u16, _v: u32) {}
}


#[repr(C)]
pub struct ulldiv_t { quo: u64, rem: u64, }
#[no_mangle]
#[linkage="external"]
extern fn __aeabi_uldivmod(mut n: u64, mut d: u64) -> ulldiv_t {
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
