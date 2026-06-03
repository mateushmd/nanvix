use core::{
	cell::UnsafeCell,
	ptr
};

// #[derive(Default, Clone)]
#[repr(transparent)]
pub struct VolatileCell<T> {
	value: UnsafeCell<T>
}

impl<T> VolatileCell<T> {
	/*
	pub const fn new(value: T) -> Self {
		Self {
			value: UnsafeCell::new(value)
		}
	}
	*/

	pub fn read(&self) -> T {
		unsafe { ptr::read_volatile(self.value.get()) }
	}

	pub fn write(&mut self, value: T) {
		unsafe { ptr::write_volatile(self.value.get(), value) }
	}
}
