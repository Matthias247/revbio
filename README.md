revbio
======

revbio is a Rust library which provides event based I/O.

It provides the ability to use multiple I/O objects like Sockets, Signals, Timers or Channels in a single task parallel.
`revbio` is based on `Events` that signal the new status of these objects.

revbio is currently an experiment. Use it at your own risk. All APIs may heavily change.

Supported platforms
-------------------
revio works only inside Rusts native tasks. 
In addition to that currently linux is the only supported platform.

Building revbio
---------------
Simply execute `rustc lib.rs`

Example
-------
For an example see example.rs in the repository