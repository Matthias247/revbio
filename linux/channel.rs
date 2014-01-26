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
use std::unstable::mutex::Mutex;
use std::sync::arc::UnsafeArc;
use extra::ringbuf::RingBuf;
use extra::container::Deque;

use super::events;
use super::eventqueue::EventQueue;
use super::eventqueueimpl::EventQueueImpl;
use super::syscalls;
use super::helpers;

pub struct BlockingReceiver<T> {
	priv data: UnsafeArc<SharedChannelData<T>>
}

pub struct Transmitter<T> {
	priv data: UnsafeArc<SharedChannelData<T>>
}

struct SharedChannelData<T> {
	queue: RingBuf<T>,
	mutex: Mutex,
	port_alive: bool,
	nr_senders: uint,
	receiver_notified: bool,
	epoll_fd: i32
}

pub struct Channel<T>;

impl <T:Send> Channel<T> {
	pub fn create_blocking() -> (BlockingReceiver<T>, Transmitter<T>) {
		let shared_data: UnsafeArc<SharedChannelData<T>> 
			= UnsafeArc::new(SharedChannelData {
				queue: RingBuf::new(),
				mutex: unsafe { Mutex::new() },
				port_alive: true,
				receiver_notified: false,
				nr_senders: 1,
				epoll_fd: -1
		});
		(BlockingReceiver{data: shared_data.clone()}, Transmitter{data: shared_data})
	}

	pub fn create(event_queue: &EventQueue) -> (~Receiver<T>, Transmitter<T>) {
		let (rx,tx) = Channel::<T>::create_blocking();
		(Receiver::from_blocking_receiver(rx, event_queue), tx)
	}
}

impl<T:Send> BlockingReceiver<T> {
	pub fn recv(&self) -> T {
		let data = self.data.get();
		unsafe {
			(*data).mutex.lock();
			while (((*data).queue.len() < 1) && ((*data).nr_senders != 0)) {
				(*data).mutex.wait();
			}
			if (*data).queue.len() > 0 {
				let ret = (*data).queue.pop_front().unwrap();
				(*data).receiver_notified = false;
				(*data).mutex.unlock();
				ret
			}
			else { // Sender(s) dead
				(*data).mutex.unlock();
				fail!("Remote channels are dead");
			}
		}
	}

	pub fn recv_opt(&self) -> Option<T> {
		let data = self.data.get();
		let mut ret = None;
		unsafe {
			(*data).mutex.lock();
			while (((*data).queue.len() < 1) && ((*data).nr_senders != 0)) {
				(*data).mutex.wait();
			}
			if (*data).queue.len() > 0 {
				ret = Some((*data).queue.pop_front().unwrap());
			}
			(*data).receiver_notified = false;
			(*data).mutex.unlock();
			ret						
		}
	}

	pub fn try_recv(&self) -> TryRecvResult<T> {
		let data = self.data.get();
		let mut ret = Empty;
		unsafe {
			(*data).mutex.lock();
			if (*data).queue.len() >= 1 {
				ret = Data((*data).queue.pop_front().unwrap());
			}
			else if (*data).nr_senders == 0 {
				ret = Disconnected;
			}
			(*data).receiver_notified = false;
			(*data).mutex.unlock();
			ret						
		}
	}
}

#[unsafe_destructor]
impl<T:Send> Drop for BlockingReceiver<T> {
	fn drop(&mut self) {
		let data = self.data.get();
		unsafe {
			(*data).mutex.lock();
			(*data).port_alive = false;
			(*data).queue.clear();
			(*data).mutex.unlock();
		}
	}
}

pub enum TryRecvResult<T> {
    Empty,
    Disconnected,
    Data(T),
}

pub struct Receiver<T> {
	priv process_func: fn(func_ptr: *libc::c_void, event_queue: &mut EventQueueImpl, epoll_events: u32),
	priv receiver: BlockingReceiver<T>,
	priv event_queue: Rc<RefCell<EventQueueImpl>>,
	priv event_source_id: events::EventSourceId,
	priv epoll_events: u32,
	priv available_messages: uint
}

impl<T> events::EventSource for Receiver<T> {
	fn is_source_of(&self, event: &events::Event) -> bool {
		if self.event_source_id == event.source {true}
		else { false }
	}
}

impl<T:Send> Receiver<T> {
	pub fn from_blocking_receiver(blocking_receiver: BlockingReceiver<T>, event_queue: &EventQueue) -> ~Receiver<T> {
		let receiver = ~Receiver{
			receiver: blocking_receiver,
			event_queue: event_queue._get_impl(),
			process_func: Receiver::<T>::process_epoll_events,
			event_source_id: events::EventSourceId::new(),
			epoll_events: 0,
			available_messages: 0
		};

		let data = receiver.receiver.data.get();
		unsafe {
			(*data).mutex.lock();

			if (*data).nr_senders != 0 { // Don't need to do anything when there are 0 senders
				let fd = syscalls::eventfd(0, 0);
				if fd == -1 {
					(*data).mutex.unlock();
					fail!("Creating eventfd for port failed: {}", helpers::last_error().desc);
				}
				(*data).epoll_fd = fd;
				let callback: *libc::c_void = cast::transmute(&receiver.process_func);
				receiver.event_queue.borrow().with_mut(|q|
					q.register_fd(fd, syscalls::EPOLLIN, callback)
				);
				// Check if there are messages available and if yes queue them
				if (*data).queue.len() > 0 {
					let bytes = [0,..8];
					let content: *mut u64 = cast::transmute(&bytes);
					*content = 1;
					let ret = helpers::retry(||
						libc::write(fd, bytes.as_ptr() as *libc::c_void, 8) as libc::c_int
					);
					if ret == -1 {
						(*data).mutex.unlock();
						fail!("Error on writing to eventfd: {}", helpers::last_error().desc);
					}
					(*data).receiver_notified = true;
				}
			}
			else {
				receiver.event_queue.borrow().with_mut(|q|
					q.ready_events.push_back(events::Event{
						event_type: events::ChannelClosedEvent,
						is_valid: true,
						source: receiver.event_source_id.clone()
					})
				);
			}

			(*data).mutex.unlock();
		}
		receiver
	}

	pub fn recv(&mut self) -> Option<T> {
		if self.available_messages > 0 {
			let ret = self.receiver.recv();
			self.available_messages -= 1;
			Some(ret)
		}
		else {
			None
		}
	}

	fn process_epoll_events(func_ptr: *libc::c_void, event_queue: &mut EventQueueImpl, epoll_events: u32) {
		unsafe {
			let receiver: *mut Receiver<T> = func_ptr as *mut Receiver<T>;
			let data = (*receiver).receiver.data.get();

			if (epoll_events & syscalls::EPOLLIN != 0) {
				let buffer = [0, ..8];

				let ret = helpers::retry(||
					libc::read((*data).epoll_fd, 
						       buffer.as_ptr() as *mut libc::c_void,
						       buffer.len() as libc::size_t) as i32
				);

				if ret == 8 { // Must be 8 bytes
					let value: *u64 = cast::transmute(&buffer);
					if *value == 1 {
						(*data).mutex.lock();
						let new_messages = (*data).queue.len() - (*receiver).available_messages;
						(*receiver).available_messages += (*data).queue.len();
						for _ in range(0, new_messages) {
							let e = events::Event {
								event_type: events::ChannelMessageEvent,
								is_valid: true,
								source: (*receiver).event_source_id.clone()
							};
							event_queue.ready_events.push_back(e);
						}
						if (*data).nr_senders == 0 {
							let e = events::Event {
								event_type: events::ChannelClosedEvent,
								is_valid: true,
								source: (*receiver).event_source_id.clone()
							};
							event_queue.ready_events.push_back(e);
						}
						(*data).receiver_notified = false;
						(*data).mutex.unlock();
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
impl<T:Send> Drop for Receiver<T> {
	fn drop(&mut self) {
		let data = self.receiver.data.get();
		unsafe {
			(*data).mutex.lock();
			if (*data).epoll_fd != -1 {
				libc::close((*data).epoll_fd);
				(*data).epoll_fd = -1; // Disable further events from clients
			}
			(*data).mutex.unlock();
		}
		self.remove_pending_events();
	}
}

impl<T:Send> Transmitter<T> {

	pub fn send(&self, t: T) {
		self.try_send(t);
	}

	pub fn try_send(&self, t: T) -> bool {
		let data = self.data.get();
		unsafe {
			(*data).mutex.lock();
			if !(*data).port_alive {
				(*data).mutex.unlock();
				return false; 
			}
			(*data).queue.push_back(t);
			if (*data).queue.len() == 1 && !(*data).receiver_notified { // Signal to receiver
				// Decide depending on port state what to do
				if (*data).epoll_fd == -1 {
					(*data).mutex.signal();
				}
				else {
					let bytes = [0,..8];
					let content: *mut u64 = cast::transmute(&bytes);
					*content = 1;
					let ret = helpers::retry(||
						libc::write((*data).epoll_fd, bytes.as_ptr() as *libc::c_void, 8) as libc::c_int
					);
					if ret == -1 {
						(*data).mutex.unlock();
						fail!("Error on writing to eventfd: {}", helpers::last_error().desc);
					}
				}
				(*data).receiver_notified = true;
			}
			(*data).mutex.unlock();			
		}
		true
	}
}

#[unsafe_destructor]
impl<T:Send> Drop for Transmitter<T> {
	fn drop(&mut self) {
		
		// Decrease refcount
		let data = self.data.get();
		unsafe {
			(*data).mutex.lock();
			(*data).nr_senders -= 1;
			if (*data).nr_senders == 0 && !(*data).receiver_notified {
				if (*data).epoll_fd == -1 {
					(*data).mutex.signal();
				}
				else {
					let bytes = [0,..8];
					let content: *mut u64 = cast::transmute(&bytes);
					*content = 1;
					let ret = helpers::retry(||
						libc::write((*data).epoll_fd, bytes.as_ptr() as *libc::c_void, 8) as libc::c_int
					);
					if ret == -1 {
						(*data).mutex.unlock();
						fail!("Error on writing to eventfd: {}", helpers::last_error().desc);
					}
				}
				(*data).receiver_notified = true;

			}
			(*data).mutex.unlock();
		}
	}
}

impl<T:Send> Clone for Transmitter<T> {
	fn clone(&self) -> Transmitter<T> {
		let new = Transmitter{data: self.data.clone()};
		// Increase refcount
		let data = self.data.get();
		unsafe {
			(*data).mutex.lock();
			(*data).nr_senders += 1;
			(*data).mutex.unlock();
		}
		new
	}
}