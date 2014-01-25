// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::libc;

/// Epoll typess
pub static EPOLLIN: u32 = 0x001;
pub static EPOLLPRI: u32 = 0x002;
pub static EPOLLOUT: u32 = 0x004;
pub static EPOLLRDNORM: u32 = 0x040;
pub static EPOLLRDBAND: u32 = 0x080;
pub static EPOLLWRNORM: u32 = 0x100;
pub static EPOLLWRBAND : u32 = 0x200;
pub static EPOLLMSG: u32 = 0x400;
pub static EPOLLERR: u32 = 0x008;
pub static EPOLLHUP: u32 = 0x010;
pub static EPOLLONESHOT: u32 = (1 << 30);
pub static EPOLLET: u32 = (1 << 31);

pub static EPOLL_CTL_ADD: i32 = 1;	/* Add a file decriptor to the interface.  */
pub static EPOLL_CTL_DEL: i32 = 2;	/* Remove a file decriptor from the interface.  */
pub static EPOLL_CTL_MOD: i32 = 3;	/* Change file decriptor epoll_event structure.  */

/// Structure that allows to store user data during an epoll operation
pub struct epoll_data {
	data: [u8, ..8]
}

impl epoll_data {
	pub fn new() -> epoll_data {
		epoll_data {data: [0, ..8]}
	}

	pub fn set_data_as_u64(&mut self, data:u64) {
		let start: *mut epoll_data = self;
		unsafe { *(start as *mut u64) = data; }
	}
	pub fn as_u64(&self) -> u64 {
		let start: *epoll_data = self;
		unsafe { *(start as *u64) }
	}
	pub fn set_data_as_u32(&mut self, data: u32) {
		self.data = [0, ..8];
		let start: *mut epoll_data = self;
		unsafe { *(start as *mut u32) = data; }
	}
	pub fn get_data_as_u32(&self) -> u32 {
		let start: *epoll_data = self;
		unsafe { *(start as *u32) }
	}
	pub fn set_data_as_fd(&mut self, fd: i32) {
		self.data = [0, ..8];
		let start: *mut epoll_data = self;
		unsafe { *(start as *mut i32) = fd; }
	}
	pub fn get_data_as_fd(&self) -> i32 {
		let start: *epoll_data = self;
		unsafe { *(start as *i32) }
	}
	pub fn set_data_as_ptr(&mut self, ptr: *libc::c_void) {
		self.data = [0, ..8];
		let start: *mut epoll_data = self;
		unsafe { *(start as *mut* libc::c_void) = ptr; }
	}
	pub fn get_data_as_ptr(&self) -> *libc::c_void {
		let start: *epoll_data = self;
		unsafe { *(start as **libc::c_void) }
	}
}

pub struct epoll_event {
	/// Epoll events
	events: u32,
	/// Epoll user data
	data: epoll_data
}

impl epoll_event {
	pub fn new() -> epoll_event {
		epoll_event{
			events: 0,
			data: epoll_data::new()
		}
	}
}

extern {
	pub fn epoll_create(size: i32) -> i32;
	pub fn epoll_ctl(epfd: i32, op: i32, fd: i32, event: *epoll_event) -> i32; 
	pub fn epoll_wait(epfd: i32, events: *epoll_event,
			   maxevents: i32, timeout: i32) -> i32;
}

/// Timerfd calls
extern {
	pub fn timerfd_create(clockid: i32, flags: i32) -> i32;
    pub fn timerfd_settime(fd: i32, flags: i32,
                       new_value: *itimerspec,
                       old_value: *itimerspec) -> i32;
	pub fn timerfd_gettime(fd: i32, curr_value: *itimerspec) -> i32;
}

pub struct itimerspec {
	it_interval: libc::timespec,	/* Interval for periodic timer */
	it_value: libc::timespec		/* Initial expiration */
}

impl itimerspec {
	pub fn new() -> itimerspec {
		itimerspec {
			it_interval: libc::timespec {
				tv_sec: 0,
				tv_nsec: 0
			},
			it_value: libc::timespec {
				tv_sec: 0,
				tv_nsec: 0
			}
		}
	}
}

extern {
	pub fn eventfd(initval: u32, flags: i32) -> i32;
	pub fn ioctl(fd: i32, req: i32, ...) -> i32;
	pub fn fcntl(fd: i32, cmd:i32, ...) -> i32;

	pub fn getsockopt(socket: libc::c_int, level: libc::c_int, name: libc::c_int,
					  value: *libc::c_void, option_len: *libc::socklen_t) -> libc::c_int;
}

pub fn set_fd_blocking(fd: i32, blocking: bool) {
	unsafe {
		let mut flags = fcntl(fd, F_GETFL, 0);
		if (blocking) {
			flags = flags & !O_NONBLOCK;
		}
		else {
			flags = flags |O_NONBLOCK;
		}
		fcntl(fd, F_SETFL, flags);
	}
}

pub static FIONREAD: i32 = 0x541B;
pub static O_NONBLOCK: i32 = 0x800;
pub static F_GETFL: i32 = 3;	/* Get file status flags.  */
pub static F_SETFL: i32 = 4;	/* Set file status flags.  */

pub static SOCK_CLOEXEC: i32 = 0x80000;	/* Atomically set close-on-exec flag for the new descriptor(s).  */
pub static SOCK_NONBLOCK: i32 = 0x800; /* Atomically mark descriptor(s) as non-blocking.  */

pub static SO_ERROR: i32 = 4;