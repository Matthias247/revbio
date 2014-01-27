// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cell::RefCell;
use std::rc::Rc;

use super::events;
use super::eventqueueimpl;
use super::IoResult;

pub struct EventQueue {
	priv queue: Rc<RefCell<eventqueueimpl::EventQueueImpl>>
}

impl EventQueue {
	pub fn new() -> EventQueue {		
		EventQueue{
			queue: Rc::new(RefCell::new(eventqueueimpl::EventQueueImpl::new()))
		}
	}

	pub fn _get_impl(&self) -> Rc<RefCell<eventqueueimpl::EventQueueImpl>> {
		self.queue.clone()
	}

	pub fn next_event(&mut self) -> IoResult<events::Event> {
		self.queue.borrow().with_mut(|ev_queue|ev_queue.next_event())
	}
}

pub trait IEventQueue {
	fn push_back_event(&mut self, event: events::Event);
	fn push_front_event(&mut self, event: events::Event);
}

impl IEventQueue for EventQueue {
	#[inline]
	fn push_back_event(&mut self, event: events::Event) {
		let mut refmut = self.queue.borrow().borrow_mut();
		refmut.get().push_back_event(event);
	}
	#[inline]
	fn push_front_event(&mut self, event: events::Event) {
		let mut refmut = self.queue.borrow().borrow_mut();
		refmut.get().push_front_event(event);
	}
}