#[no_uv];
extern mod native;
extern mod revbio;
extern mod extra;

use extra::time;

use revbio::events;
use revbio::channel::{Channel,BlockingReceiver,Receiver,Transmitter};
use revbio::EventQueue;

#[start]
fn start(argc: int, argv: **u8) -> int {
	native::start(argc, argv, proc() {
		bench_conditions_main();
	})
}

fn timediff(start: time::Timespec, end: time::Timespec) -> f64 {
	let mut diff:f64;	

	if start.sec == end.sec {
		diff = ((end.nsec - start.nsec) as f64) / 1000000000f64;
	}
	else {
		diff = ((1000000000i32 - start.nsec) as f64) / 1000000000f64;
		let startsec = start.sec + 1;
		diff += ((end.sec - startsec) as f64);
		diff += ((end.nsec) as f64) / 1000000000f64;
	}
	diff
}


fn bench_conditions_main() {
	let ITERATIONS = 100000u32;
	
	let (port,chan): (Port<i32>, Chan<i32>) = Chan::new();
	let start_time = time::get_time();

	native::task::spawn(proc() { 
		for _ in range(0,ITERATIONS) {
			chan.send(0);
		}
	});

	for _ in range(0,ITERATIONS) {
		port.recv();
	}

	let end_time = time::get_time();
	let diff = timediff(start_time, end_time);
	println!("Native channels: Diff: {:?}", diff);

	let (rx,tx): (BlockingReceiver<i32>, Transmitter<i32>) = Channel::create_blocking();
	let start_time = time::get_time();

	native::task::spawn(proc() {
		for _ in range(0,ITERATIONS) {
			tx.send(0);
		}
	});

	for _ in range(0,ITERATIONS) {
		rx.recv();
	}

	let end_time = time::get_time();
	let diff = timediff(start_time, end_time);
	println!("Revio blocking channels: Diff: {:?}", diff);

	let mut ev_queue = EventQueue::new();
	let (mut rx,tx): (~Receiver<i32>, Transmitter<i32>) = Channel::create(&ev_queue);
	let mut nr_received = 0u32;
	let start_time = time::get_time();

	native::task::spawn(proc() {
		for _ in range(0,ITERATIONS) {
			tx.send(0);
		}
	});

	loop {
		let event = ev_queue.next_event().unwrap();			
		
		if event.originates_from(rx) {
			match event.event_type {
				events::ChannelMessageEvent => {
					rx.recv();
					nr_received += 1;
					if nr_received == ITERATIONS {
						break;
					}
				},
				_ => ()
			}
		}
	}	

	let end_time = time::get_time();
	let diff = timediff(start_time, end_time);
	println!("Revio eventbased channels: Diff: {:?}", diff);
}