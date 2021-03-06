//
//
//

use memory::virt::ProtectionMode;
use arch::memory::PAddr;

static S_TEMP_MAP_SEMAPHORE: ::sync::Semaphore = ::sync::Semaphore::new(1022, 1023);
const KERNEL_TEMP_BASE : usize = 0xFFC00000;
const USER_BASE_TABLE: usize = 0x7FFF_E000;	// 2GB - 0x800 * 4 = 0x2000
const USER_TEMP_TABLE: usize = 0x7FFF_D000;	// Previous page to that
const USER_TEMP_BASE: usize = 0x7FC0_0000;	// 1 page worth of temp mappings (top three are used for base table/temp table)

pub fn post_init() {
	kernel_table0[0].store(0);
}

fn prot_mode_to_flags(mode: ProtectionMode) -> u32 {
	//AP[2] = 9, AP[1:0] = 5:4, XN=0, SmallPage=1
	match mode
	{
	ProtectionMode::Unmapped => 0x000,
	ProtectionMode::KernelRO => 0x213,
	ProtectionMode::KernelRW => 0x013,
	ProtectionMode::KernelRX => 0x052,
	ProtectionMode::UserRO => 0x233,	// 1,11,1
	ProtectionMode::UserRW => 0x033,	// 0,11,1
	ProtectionMode::UserRX => 0x232,	// 1,11,0
	ProtectionMode::UserRWX => 0x032,	// 0,11,0
	ProtectionMode::UserCOW => 0x223,	// 1,10,1 is a deprecated encoding for ReadOnly, need to find a better encoding
	}
}
fn flags_to_prot_mode(flags: u32) -> ProtectionMode {
	match flags
	{
	0x000 => ProtectionMode::Unmapped,
	0x212 => ProtectionMode::KernelRO,
	0x012 => ProtectionMode::KernelRW,
	0x053 => ProtectionMode::KernelRX,
	0x232 => ProtectionMode::UserRO,
	0x032 => ProtectionMode::UserRW,
	0x233 => ProtectionMode::UserRX,
	0x033 => ProtectionMode::UserRWX,
	0x223 => ProtectionMode::UserCOW,
	v @ _ => todo!("Unknown mode value {:#x}", v),
	}
}

/// Atomic 32-bit integer, used for table entries
#[repr(C)]
struct AtomicU32(::core::cell::UnsafeCell<u32>);
impl AtomicU32 {
	/// Compare and exchange, returns old value and writes `new` if it was equal to `val`
	pub fn cxchg(&self, val: u32, new: u32) -> u32 {
		// SAFE: Atomic
		unsafe { ::core::intrinsics::atomic_cxchg_relaxed(self.0.get(), val, new) }
	}
	/// Exchange
	pub fn xchg(&self, new: u32) -> u32 {
		// SAFE: Atomic
		unsafe { ::core::intrinsics::atomic_xchg_relaxed(self.0.get(), new) }
	}
	/// Unconditionally stores
	pub fn store(&self, val: u32) {
		// SAFE: Atomic
		unsafe { ::core::intrinsics::atomic_store_relaxed(self.0.get(), val) }
	}
	/// Unconditionally loads
	pub fn load(&self) -> u32 {
		// SAFE: Atomic
		unsafe { ::core::intrinsics::atomic_load_relaxed(self.0.get()) }
	}
}

pub fn is_fixed_alloc<T>(addr: *const T, size: usize) -> bool {
	const BASE: usize = super::addresses::KERNEL_BASE;
	const ONEMEG: usize = 1024*1024;
	const LIMIT: usize = super::addresses::KERNEL_BASE + 4*ONEMEG;
	let addr = addr as usize;
	if BASE <= addr && addr < LIMIT {
		if addr + size <= LIMIT {
			true
		}
		else {
			false
		}
	}
	else {
		false
	}
}
// UNSAFE: Can cause aliasing
pub unsafe fn fixed_alloc(_p: PAddr, _count: usize) -> Option<*mut ()> {
	None
}

#[derive(Copy,Clone,Debug)]
enum PageEntryRegion {
	NonGlobal,
	Global,
}
impl PageEntryRegion {
	fn get_section_ent(&self, idx: usize) -> &AtomicU32 {
		assert!(idx < 4096);
		match self
		{
		&PageEntryRegion::NonGlobal => todo!("PageEntryRegion::get_section_ent - non-global"),
		&PageEntryRegion::Global => &kernel_table0[idx],
		}
	}
}
enum PageEntry {
	Section {
		rgn: PageEntryRegion,
		idx: usize,
		ofs: usize
		},
	Page {
		mapping: TempHandle<AtomicU32>,
		idx: usize,
		ofs: usize
		},
}
impl PageEntry
{
	fn alloc(addr: *const (), level: usize) -> Result<PageEntry, ()> {
		todo!("PageEntry::alloc({:p}, level={})", addr, level);
	}
	/// Obtain a page entry for the specified address
	fn get(addr: *const ()) -> PageEntry {
		use super::addresses::KERNEL_BASE;
		let (rgn, p_idx) = if (addr as usize) < KERNEL_BASE {
				(PageEntryRegion::NonGlobal, (addr as usize - KERNEL_BASE) >> 12)
			}
			else {
				(PageEntryRegion::Global, (addr as usize) >> 12)
			};

		// SAFE: Aliasing in this case is benign
		let sect_ent = rgn.get_section_ent(p_idx >> 8).load();
		if sect_ent & 0b11 == 0b01 {
			PageEntry::Page {
				// SAFE: Alias is beign, as accesses are atomic
				mapping: unsafe { TempHandle::new( sect_ent & !0xFFF ) },
				idx: p_idx,
				ofs: (addr as usize) & 0xFFF,
				}
		}
		else {
			PageEntry::Section {
				rgn: rgn,
				idx: p_idx >> 8,
				ofs: (addr as usize) & 0xFF_FFF,
				}
		}
	}


	fn is_reserved(&self) -> bool {
		match self
		{
		&PageEntry::Section { rgn, idx, .. } => (rgn.get_section_ent(idx).load() & 3 != 0),
		&PageEntry::Page { ref mapping, idx, .. } => (mapping[idx & 0x3FF].load() & 3 != 0),
		}
	}

	fn phys_addr(&self) -> ::arch::memory::PAddr {
		match self
		{
		&PageEntry::Section { rgn, idx, ofs } => (rgn.get_section_ent(idx).load() & !0xFFF) + ofs as u32,
		&PageEntry::Page { ref mapping, idx ,ofs } => (mapping[idx & 0x3FF].load() & !0xFFF) + ofs as u32,
		}
	}
	fn mode(&self) -> ::memory::virt::ProtectionMode {
		match self
		{
		&PageEntry::Section { rgn, idx, .. } =>
			match rgn.get_section_ent(idx).load() & 0xFFF
			{
			0x000 => ProtectionMode::Unmapped,
			0x402 => ProtectionMode::KernelRW,
			v @ _ if v & 3 == 1 => unreachable!(),
			v @ _ => todo!("Unknown mode value in section {:?} {} - {:#x}", rgn, idx, v),
			},
		&PageEntry::Page { ref mapping, idx, .. } => flags_to_prot_mode( mapping[idx & 0x3FF].load() & 0xFFF ),
		}
	}
	//fn reset(&mut self) -> Option<(::arch::memory::PAddr, ::memory::virt::ProtectionMode)> {
	//}
}

extern "C" {
	static kernel_table0: [AtomicU32; 0x800*2];
	static kernel_exception_map: [AtomicU32; 1024];
}

/// Returns the physical address of the table controlling `vaddr`. If `alloc` is true, a new table will be allocated if needed.
fn get_table_addr<T>(vaddr: *const T, alloc: bool) -> Option< (::arch::memory::PAddr, usize) > {
	let addr = vaddr as usize;
	let page = addr >> 12;
	let (ttbr_ofs, tab_idx) = (page >> 10, page & 0x3FF);
	let ent_r = if ttbr_ofs < 0x800/4 {
			// SAFE: This memory should always be mapped
			unsafe { & (*(USER_BASE_TABLE as *const [AtomicU32; 0x800]))[ttbr_ofs*4 .. ][..4] }
		}
		else {
			// Kernel
			&kernel_table0[ ttbr_ofs*4 .. ][..4]
		};
	
	let ent_v = ent_r[0].load();
	match ent_v & 0xFFF
	{
	0 => if alloc {
			let frame = ::memory::phys::allocate_bare().expect("TODO get_table_addr - alloc failed");
			//::memory::virt::with_temp(|frame: &[AtomicU32]| for v in frame.iter { v.store(0) });
			// SAFE: Unaliased memory
			for v in unsafe { TempHandle::<AtomicU32>::new( frame ) }.iter() {
				v.store(0);
			}
			let ent_v = ent_r[0].cxchg(0, frame + 0x1);
			if ent_v != 0 {
				::memory::phys::deref_frame(frame);
				Some( (ent_v & !0xFFF, tab_idx) )
			}
			else {
				ent_r[1].store(frame + 0x400 + 0x1);
				ent_r[2].store(frame + 0x800 + 0x1);
				ent_r[3].store(frame + 0xC00 + 0x1);
				Some( (frame & !0xFFF, tab_idx) )
			}
		}
		else {
			None
		},
	1 => Some( (ent_v & !0xFFF, tab_idx) ),
	v @ _ => todo!("get_table_addr - Other flags bits {:#x}", v),
	}
}

/// Handle to a temporarily mapped frame
pub struct TempHandle<T>(*mut T);
impl<T> TempHandle<T>
{
	/// UNSAFE: User must ensure that address is valid, and that no aliasing occurs
	pub unsafe fn new(phys: ::arch::memory::PAddr) -> TempHandle<T> {
		//log_trace!("TempHandle<{}>::new({:#x})", type_name!(T), phys);
		let val = (phys as u32) + 0x13;	

		S_TEMP_MAP_SEMAPHORE.acquire();
		// #1023 is reserved for -1 mapping
		for i in 0 .. 1023 {
			if kernel_exception_map[i].cxchg(0, val) == 0 {
				let addr = (KERNEL_TEMP_BASE + i * 0x1000) as *mut _;
				tlbimva(addr as *mut ());
				//log_trace!("- Addr = {:p}", addr);
				return TempHandle( addr );
			}
		}
		panic!("No free temp mappings");
	}
}
impl<T> ::core::ops::Deref for TempHandle<T> {
	type Target = [T];
	fn deref(&self) -> &[T] {
		// SAFE: We should have unique access
		unsafe { ::core::slice::from_raw_parts(self.0, 0x1000 / ::core::mem::size_of::<T>()) }
	}
}
impl<T> ::core::ops::DerefMut for TempHandle<T> {
	fn deref_mut(&mut self) -> &mut [T] {
		// SAFE: We should have unique access
		unsafe { ::core::slice::from_raw_parts_mut(self.0, 0x1000 / ::core::mem::size_of::<T>()) }
	}
}
impl<T> ::core::ops::Drop for TempHandle<T> {
	fn drop(&mut self) {
		let i = (self.0 as usize - KERNEL_TEMP_BASE) / 0x1000;
		kernel_exception_map[i].store(0);
		S_TEMP_MAP_SEMAPHORE.release();
	}
}

pub fn is_reserved<T>(addr: *const T) -> bool {
	get_phys_opt(addr).is_some()
	//PageEntry::get(addr as *const ()).is_reserved()
}
pub fn get_phys<T>(addr: *const T) -> ::arch::memory::PAddr {
	get_phys_opt(addr).unwrap_or(0)
	//PageEntry::get(addr as *const ()).phys_addr()
}
fn get_phys_opt<T>(addr: *const T) -> Option<::arch::memory::PAddr> {
	let res: u32;
	// SAFE: Correct register accesses
	unsafe {
		// TODO: Disable interrupts during this operation
		asm!("
			mcr p15,0, $1, c7,c8,0;
			isb;
			mrc p15,0, $0, c7,c4,0
			"
			: "=r" (res) : "r" (addr)
			);
	};

	match res & 3 {
	1 | 3 => None,
	0 => Some( (res & !0xFFF) | (addr as usize as u32 & 0xFFF) ),
	2 => {
		todo!("Unexpected supersection at {:p}, res={:#x}", addr, res);
		let pa_base: u64 = (res & !0xFFFFFF) as u64 | ((res as u64 & 0xFF0000) << (32-16));
		Some( pa_base as u32 | (addr as usize as u32 & 0xFFFFFF) )
		},
	_ => unreachable!(),
	}
}

pub fn get_info<T>(addr: *const T) -> Option<(::arch::memory::PAddr, ::memory::virt::ProtectionMode)> {
	let pe = PageEntry::get(addr as *const ());
	if pe.is_reserved() {
		Some( (pe.phys_addr(), pe.mode()) )
	}
	else {
		None
	}
}

/// TLB Invalidate by Modified Virtual Address
fn tlbimva(a: *mut ()) {
	// SAFE: TLB invalidation is not the unsafe part :)
	unsafe {
		asm!("mcr p15,0, $0, c8,c7,1 ; dsb ; isb" : : "r" ( (a as usize & !0xFFF) | 1 ) : "memory" : "volatile")
	}
}
///// Data Cache Clean by Modified Virtual Address (to PoC)
//fn dccmvac(a: *mut ()) {
//}

pub unsafe fn map(a: *mut (), p: PAddr, mode: ProtectionMode) {
	map_int(a,p,mode)
}
fn map_int(a: *mut (), p: PAddr, mode: ProtectionMode) {
	// 1. Map the relevant table in the temp area
	let (tab_phys, idx) = get_table_addr(a, true).unwrap();
	//log_debug!("map_int({:p}, {:#x}, {:?}) - tab_phys={:#x},idx={}", a, p, mode, tab_phys, idx);
	// SAFE: Address space is valid during manipulation, and alias is benign
	let mh: TempHandle<AtomicU32> = unsafe {  TempHandle::new( tab_phys ) };
	assert!(mode != ProtectionMode::Unmapped, "Invalid pass of ProtectionMode::Unmapped to map");
	// 2. Insert
	let mode_flags = prot_mode_to_flags(mode);
	//log_debug!("map(): a={:p} mh={:p} idx={}, new={:#x}", a, &mh[0], idx, p + mode_flags);
	let old = mh[idx].cxchg(0, p + mode_flags);
	assert!(old == 0, "map() called over existing allocation: a={:p}, old={:#x}", a, old);
	tlbimva(a);
}
pub unsafe fn reprotect(a: *mut (), mode: ProtectionMode) {
	// 1. Map the relevant table in the temp area
	let (tab_phys, idx) = get_table_addr(a, true).unwrap();
	// SAFE: Address space is valid during manipulation, and alias is benign
	let mh: TempHandle<AtomicU32> = unsafe { TempHandle::new( tab_phys ) };
	assert!(mode != ProtectionMode::Unmapped, "Invalid pass of ProtectionMode::Unmapped to map");
	// 2. Insert
	let mode_flags = prot_mode_to_flags(mode);
	let v = mh[idx].load();
	assert!(v != 0, "reprotect() called on an unmapped location: a={:p}", a);
	//log_debug!("reprotect(): a={:p} mh={:p} idx={}, new={:#x}", a, &mh[0], idx, (v & !0xFFF) + mode_flags);
	let old = mh[idx].cxchg(v, (v & !0xFFF) + mode_flags);
	assert!(old == v, "reprotect() called in a racy manner: a={:p} old({:#x}) != v({:#x})", a, old, v);
	tlbimva(a);
}
pub unsafe fn unmap(a: *mut ()) -> Option<PAddr> {
	// 1. Map the relevant table in the temp area
	let (tab_phys, idx) = get_table_addr(a, true).unwrap();
	// SAFE: Address space is valid during manipulation, and alias is benign
	let mh: TempHandle<AtomicU32> = unsafe { TempHandle::new( tab_phys ) };
	let old = mh[idx].xchg(0);
	tlbimva(a);
	if old & 3 == 0 {
		None
	}
	else {
		Some( old & !0xFFF )
	}
}

#[derive(Debug)]
pub struct AddressSpace(u32);
impl AddressSpace
{
	pub fn pid0() -> AddressSpace {
		extern "C" {
			static kernel_table0: ::Void;
			static kernel_phys_start: u32;
		}
		let tab0_addr = kernel_phys_start + (&kernel_table0 as *const _ as usize as u32 - 0x80000000);
		AddressSpace( tab0_addr )
	}
	pub fn new(clone_start: usize, clone_end: usize) -> Result<AddressSpace,::memory::virt::MapError> {
		todo!("AddressSpace::new({:#x} -- {:#x})", clone_start, clone_end);
	}

	pub fn get_ttbr0(&self) -> u32 { self.0 }
}

