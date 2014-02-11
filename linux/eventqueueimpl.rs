// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cast;
use std::libc;
use extra::ringbuf::RingBuf;
use extra::container::Deque;

use super::eventqueue::IEventQueue;
use super::events;
use super::IoResult;
use super::syscalls;
use super::helpers;

pub struct EventQueueImpl {
	priv fd: i32, // epoll fd,
	priv ready_events: RingBuf<events::Event>
}

impl IEventQueue for EventQueueImpl {
	#[inline]
	fn push_back_event(&mut self, event: events::Event) {
		self.ready_events.push_back(event);
	}
	#[inline]
	fn push_front_event(&mut self, event: events::Event) {
		self.ready_events.push_front(event);
	}
}

impl EventQueueImpl {

	pub fn new() -> EventQueueImpl {
		let fd = unsafe { syscalls::epoll_create(64) }; // Parameter is ignored
		if fd == 1 {
			fail!(helpers::last_error());
		}
		EventQueueImpl{
				fd: fd,
				ready_events: RingBuf::new()
		}
	}

	pub fn next_event(&mut self) -> IoResult<events::Event> {
		if self.ready_events.len() > 0 {
			Ok(self.ready_events.pop_front().unwrap())
		}
		else {
			// No handles ready. Must poll
			loop { // loop until poll returns sth. useful
				let result = self.poll_events();
				match result {
					Err(err) => { return Err(err); },
					Ok(()) => {
						if self.ready_events.len() > 0 {
							let first = self.ready_events.pop_front().unwrap();
							if first.is_valid {
								return Ok(first);
							}
						}
					}
				}
			}
		}
	}

	pub fn poll_events(&mut self) -> IoResult<()> {
		let evs = syscalls::epoll_event::new();
		let ready_fds = helpers::retry(|| unsafe {
			syscalls::epoll_wait(self.fd, &evs, 1, -1)
		});
		if ready_fds == -1 {
			return Err(helpers::last_error());
		}
		else if ready_fds == 1 {
			let ptr = evs.data.get_data_as_ptr();
			let cb: *fn(*libc::c_void, &mut EventQueueImpl, u32) 
			        = unsafe { cast::transmute(ptr) };
			unsafe { (*cb)(ptr, self, evs.events) };
		}
		Ok(())
	}

	pub fn remove_pending_events(&mut self, condition: |event: &events::Event|-> bool) {//event_source: &event::EventSource) {
		for ev in self.ready_events.mut_iter() {
			if condition(ev) {
				ev.is_valid = false;
			}
		}
	}

	pub fn register_fd(&mut self, fd: i32, flags: u32, callback: *libc::c_void) {
		// Initialize epoll data
		let mut data = syscalls::epoll_data::new();
		data.set_data_as_ptr(callback);		
		let event = syscalls::epoll_event {
			events: flags,	/* Epoll events */
			data: data	/* User data variable */
		};
		// Register at epoll
		let s = unsafe { 
			syscalls::epoll_ctl(self.fd, syscalls::EPOLL_CTL_ADD, fd, &event) 
		};
		if s != 0 {
			fail!("Could not register fd for epoll");
		}
	}

	pub fn modify_fd(&mut self, fd: i32, flags: u32, callback: *libc::c_void) {
		// Initialize epoll data
		let mut data = syscalls::epoll_data::new();
		data.set_data_as_ptr(callback);
		let event = syscalls::epoll_event {
			events: flags,	/* Epoll events */
			data: data	/* User data variable */
		};
		// Register at epoll
		let s = unsafe { 
			syscalls::epoll_ctl(self.fd, syscalls::EPOLL_CTL_MOD, fd, &event) 
		};
		if s != 0 {
			fail!("Could not modify epoll interest");
		}
	}	

	pub fn unregister_fd(&mut self, fd: i32) {
		let event = syscalls::epoll_event {
			events: 0,
			data: syscalls::epoll_data::new()
		};
		// Unregister from epoll
		let s = unsafe { 
			syscalls::epoll_ctl(self.fd, syscalls::EPOLL_CTL_DEL, fd, &event) 
		};
		if s != 0 {
			fail!("Could not remove epoll interest");
		}
	}
}

#[unsafe_destructor]
impl Drop for EventQueueImpl {
	fn drop(&mut self) {
		unsafe { libc::close(self.fd); }
	}
}