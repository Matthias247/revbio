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
use std::cell::RefCell;
use std::rc::Rc;

use super::events;
use super::IoResult;
use super::eventqueue;
use super::eventqueue::IEventQueue;
use super::eventqueueimpl::EventQueueImpl;
use super::syscalls;
use super::helpers;

pub struct Timer {
	priv process_func: fn(func_ptr: *libc::c_void, event_queue: &mut EventQueueImpl, epoll_events: u32),
	priv fd: i32,
	priv is_active: bool,
	priv interval: u32,
	priv singleshot: bool,
	priv epoll_registered: bool,
	priv event_queue: Rc<RefCell<EventQueueImpl>>,
	priv event_source_id: events::EventSourceId
}

impl Timer {
	pub fn create(event_queue: &eventqueue::EventQueue) -> IoResult<~Timer> {
		let tfd = unsafe {
			syscalls::timerfd_create(libc::CLOCK_MONOTONIC, 0)
		};
		if tfd == -1 {
			Err(helpers::last_error())
		}
		else {
			Ok(~Timer{
				fd: tfd,
				interval: 0,
				is_active: false,
				singleshot: false,
				epoll_registered: false,
				event_queue: event_queue._get_impl(),
				process_func: Timer::process_epoll_events,
				event_source_id: events::EventSourceId::new()
			})
		}
	}

	pub fn set_interval(&mut self, interval: u32) {
		self.interval = interval;
	}

	pub fn get_interal(&self) -> u32 {
		self.interval
	}

	pub fn set_singleshot(&mut self, singleshot: bool) {
		self.singleshot = singleshot;
	}

	pub fn is_singleshot(&self) -> bool {
		self.singleshot
	}

	pub fn is_active(&self) -> bool {
		self.is_active
	}

	pub fn stop(&mut self) {
		if !self.is_active { return; }

		let new_value = syscalls::itimerspec::new(); // init to 0

		let ret = unsafe {
			syscalls::timerfd_settime(self.fd, 0, &new_value, 0 as *syscalls::itimerspec)
		};
		if (ret != 0) {
			fail!("Error on stopping timer {0}", helpers::last_error().desc);
		}

		self.event_queue.borrow().with_mut(
			|q|q.unregister_fd(self.fd)
		);
		self.epoll_registered = false;
		self.is_active = false;
		self.remove_pending_events();
	}

	pub fn start(&mut self) {
		if self.is_active || self.interval == 0 { return; }

		let mut new_value = syscalls::itimerspec::new();
		new_value.it_value.tv_sec = (self.interval / 1000u32) as libc::time_t;
		new_value.it_value.tv_nsec = (self.interval % 1000u32) as libc::c_long;
		new_value.it_value.tv_nsec *= 1000000;

		if !self.singleshot {
			new_value.it_interval.tv_sec = new_value.it_value.tv_sec;
			new_value.it_interval.tv_nsec = new_value.it_value.tv_nsec;
		}

		let ret = unsafe {
			syscalls::timerfd_settime(self.fd, 0, &new_value, 0 as *syscalls::itimerspec)
		};
		if (ret != 0) {
			fail!("Error on starting timer {0}", helpers::last_error().desc);
		}

		self.is_active = true;
		// Register fd
		let epoll_flags = if self.singleshot {
			syscalls::EPOLLIN | syscalls::EPOLLONESHOT
		} else { syscalls::EPOLLIN };
		let callback: *libc::c_void = unsafe { cast::transmute(&self.process_func) };

		if !self.epoll_registered {
			self.event_queue.borrow().with_mut(|q|
				q.register_fd(self.fd, epoll_flags, callback)
			);
			self.epoll_registered = true;
		} else {
			self.event_queue.borrow().with_mut(|q|
				q.modify_fd(self.fd, epoll_flags, callback)
			);
		}
	}

	fn process_epoll_events(func_ptr: *libc::c_void, event_queue: &mut EventQueueImpl, epoll_events: u32) {
		unsafe {
			let timer: *mut Timer = func_ptr as *mut Timer;

			if (epoll_events & syscalls::EPOLLIN != 0) {
				let buffer = [0, ..8];

				let ret = helpers::retry(||
					libc::read((*timer).fd, 
						       buffer.as_ptr() as *mut libc::c_void,
						       buffer.len() as libc::size_t) as i32
				);

				if ret == 8 { // Must be 8 bytes
					let expirations: *u64 = cast::transmute(&buffer);
					for _ in range(0, *expirations) {
						let e = events::Event {
							event_type: events::TimerEvent,
							is_valid: true,
							source: (*timer).event_source_id.clone()
						};
						event_queue.push_back_event(e);
						// Set timer to inactive when it was a singleshot
						if (*timer).singleshot {
							(*timer).is_active = false;
						}
					}
				}
			}		
		}
	}

	fn remove_pending_events(&mut self) {
		self.event_queue.borrow().with_mut(|q|
	    	q.remove_pending_events(
	    		|ev|ev.source == self.event_source_id)
	    );
	}
}

#[unsafe_destructor]
impl Drop for Timer {
	fn drop(&mut self) {
		// Don't call close because this won't deque already
		// queued events if the timer is inactive
		self.remove_pending_events();
		if self.fd != 0 {
			unsafe { libc::close(self.fd); }
		}
	}
}

impl events::EventSource for Timer {
	fn get_event_source_id<'a>(&'a self) -> &'a events::EventSourceId {
		&self.event_source_id
	}
}