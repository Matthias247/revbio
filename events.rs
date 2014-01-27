// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::io::IoError;
use std::rc::Rc;

pub enum EventKind
{
	StreamClosedEvent,
	IoErrorEvent(IoError),
	DataAvailableEvent(uint),
	TimerEvent,
	ChannelClosedEvent,
	ChannelMessageEvent,
	ConnectedEvent,
	ClientConnectedEvent
}

pub struct Event
{
	event_type: EventKind,
	is_valid: bool,
	source_info: Rc<EventSourceInfo>
}

impl Event {
	pub fn originates_from<T:EventSource>(&self, source: &T) -> bool{
		if self.source_info.borrow() as *EventSourceInfo 
		   == source.get_event_source_info().borrow() as *EventSourceInfo {
		   	true
		}
		else {false}
	}
}

/**
 * A structure that stores information that belongs to the source of
 * an event
 */
pub struct EventSourceInfo {
	priv id: bool
	// More to come
}

impl EventSourceInfo {
	pub fn new() -> EventSourceInfo {
		EventSourceInfo {
			id: true
		}
	}
}

impl Clone for EventSourceInfo {
	fn clone(&self) -> EventSourceInfo {
		EventSourceInfo {
			id: self.id
		}
	}
}

/**
 * A trait that is implemented by possible sources of events
 */
pub trait EventSource
{
	fn get_event_source_info<'a>(&'a self) -> &'a Rc<EventSourceInfo>;
}