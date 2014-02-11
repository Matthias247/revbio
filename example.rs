#[no_uv];
extern mod native;
extern mod revbio;
extern mod extra;

use std::io::net::ip::{IpAddr,SocketAddr};

use revbio::events;
use revbio::channel::{Transmitter,Receiver,Channel};
use revbio::{EventQueue};
use revbio::timer::Timer;
use revbio::tcp::{TcpSocket};

#[start]
fn start(argc: int, argv: **u8) -> int {
	native::start(argc, argv, proc() {
		main();
	})
}

fn main() {
	let mut ev_queue = EventQueue::new();
	let (mut rx,tx): (~Receiver<~str>, Transmitter<~str>) = Channel::create(&ev_queue);
	
	let mut main_timer = Timer::create(&ev_queue).unwrap();
	main_timer.set_interval(2000);	
	main_timer.start();

	native::task::spawn(proc() {
		subtask(tx);
	});

	loop {
		let event = ev_queue.next_event().unwrap();			
		
		if event.originates_from(rx) {
			match event.event_type {
				events::ChannelMessageEvent => {
					let msg = rx.recv().unwrap();
					println!("Message from subtask: {}", msg);
				},
				events::ChannelClosedEvent => {
					println!("Subtask closed");
					return;
				},
				_ => ()
			}
		}
		else if event.originates_from(main_timer) {
			println!("main_timer::tick()");
		}
	}
}

fn subtask(tx: Transmitter<~str>) {
	let mut ev_queue = EventQueue::new();
	let mut sub_timer = Timer::create(&ev_queue).unwrap();
	let mut iterations = 3;
	let mut stream_alive;
	let host = ~"192.168.1.99";

	let opt_ipaddr:Option<IpAddr> = FromStr::from_str(host);
	let socketaddr = SocketAddr {ip: opt_ipaddr.unwrap(), port: 80};
	let mut socket = TcpSocket::connect(socketaddr, &ev_queue).unwrap();
	stream_alive = true;

	sub_timer.set_interval(3000);	
	sub_timer.start();

	let mut request = ~"GET / HTTP/1.1\r\nHost: ";
	request = request + "host";
	request = request + "\r\n\r\n";

	loop {
		let event = ev_queue.next_event().unwrap();
		
		if event.originates_from(sub_timer) {
			tx.send(~"subtimer::tick()");
			if !stream_alive {
				if iterations > 0 {
					socket = TcpSocket::connect(socketaddr, &ev_queue).unwrap();
					stream_alive = true;
				}
				else {
					sub_timer.stop();
					socket.close();
					return;	
				}
			}
		}
		else if event.originates_from(socket) {
			match event.event_type {
				events::ConnectedEvent => {
					tx.send(~"TCP stream got connected");
					stream_alive = true;
					let _ = socket.write(request.as_bytes());
					iterations -= 1;
				},
				events::IoErrorEvent(err) => {
					tx.send(~"IoError");
					tx.send(err.desc.to_owned());
					stream_alive = false;
					iterations -= 1;
				},
				events::StreamClosedEvent => {
					tx.send(~"TCP connection closed");
					stream_alive = false;
					iterations -= 1;
				},
				events::DataAvailableEvent(nr_bytes) => {
					let mut buffer: ~[u8] = std::vec::from_elem::<u8>(nr_bytes, 0);
					let read_res = socket.read(buffer);
					match read_res {
						Err(err) => {
							tx.send(err.desc.to_owned());
						}
						Ok(nr_read) => {
							let txt = std::str::from_utf8(buffer.slice(0, nr_read));
							if txt.is_some() {
								tx.send(txt.unwrap().to_owned());
							}
						}
					}
				},
				_ => ()
			}
		}
	}	
}