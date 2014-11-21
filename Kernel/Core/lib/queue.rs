//
//
//
#![macro_escape]
use _common::*;

pub struct Queue<T>
{
	pub head: OptPtr<QueueEnt<T>>,
	pub tail: OptMutPtr<QueueEnt<T>>,
}

pub struct QueueEnt<T>
{
	next: OptPtr<QueueEnt<T>>,
	value: T
}

pub struct Items<'s, T: 's>
{
	cur_item: Option<&'s QueueEnt<T>>,
}

impl<T> Queue<T>
{
	pub fn push(&mut self, value: T)
	{
		unsafe
		{
			let qe_ptr = ::memory::heap::alloc( QueueEnt {
				next: OptPtr(0 as *const _),
				value: value,
				} );
			
			if self.head.is_some()
			{
				assert!( self.tail.is_some() );
				let r = self.tail.as_ref().unwrap();
				assert!( r.next.is_none() );
				r.next = OptPtr(qe_ptr as *const _);
			}
			else
			{
				self.head = OptPtr(qe_ptr as *const _);
			}
			self.tail = OptMutPtr(qe_ptr);
		}
	}
	pub fn pop(&mut self) -> ::core::option::Option<T>
	{
		if self.head.is_none() {
			return None;
		}
		
		unsafe
		{
			let qe_ptr = self.head.unwrap();
			self.head = (*qe_ptr).next;
			if self.head.is_none() {
				self.tail = OptMutPtr(0 as *mut _);
			}
			
			let mut_qe_ptr = qe_ptr as *mut QueueEnt<T>;
			let rv = ::core::mem::replace(&mut (*mut_qe_ptr).value, ::core::mem::zeroed());
			::memory::heap::deallocate(qe_ptr as *mut ());
			Some(rv)
		}
	}
	
	pub fn empty(&self) -> bool
	{
		self.head.is_none()
	}
	
	pub fn items<'s>(&'s self) -> Items<'s,T>
	{
		Items {
			cur_item: unsafe { self.head.as_ref() },
		}
	}
}

impl<'s, T> Iterator<&'s T> for Items<'s,T>
{
	fn next(&mut self) -> Option<&'s T>
	{
		match self.cur_item
		{
		Some(ptr) => {
			self.cur_item = unsafe { ptr.next.as_ref() };
			Some(&ptr.value)
			},
		None => None
		}
	}
}

macro_rules! queue_init( () => (Queue{head: OptPtr(0 as *const _),tail: OptMutPtr(0 as *mut _)}) )

// vim: ft=rust

