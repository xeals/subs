# subs

`subs` is a client-server based music player for Subsonic-compliant music servers.

Its interface is modelled after `mpc`, the terminal control for the `mpd` music server.

- [Changelog](CHANGELOG.md)
- [Contributing](CONTRIBUTING.md)

# Installing

`subs` is not yet on [crates.io](https://crates.io), so installation is
currently done manually.

```sh
$ git clone https://github.com/Azphreal/subs
$ cd subs
$ cargo build --release
$ sudo mkdir -p /usr/local/bin
$ sudo cp ./target/release/subs /usr/local/bin/subs
```

# Dependencies

- rust
- cargo
- some combination of GStreamer and plugins (yet to be confirmed; apologies)

# Usage

`subs` creates a local daemon to handle playing music, sending requests to the
Subsonic server, and so on.

```sh
$ export SUBS_URL="<your Subsonic website>"
$ export SUBS_USERNAME="<your Subsonic user>"
$ export SUBS_PASSWORD="<your Subsonic password>"
$ subs daemon start
```

The daemon will start in the foreground on the current terminal. Starting the
daemon without the above environment variables will connect you to the demo
server at https://demo.subsonic.org/.

```
$ subs help
USAGE:
    subs [FLAGS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Sets the verbosity

SUBCOMMANDS:
    add        Add a song to the current playlist
    addnext    Add a song to play after the current song
    clear      Clear the current playlist
    crop       Remove all but the currently playing song
    current    Display the currently playing song
    daemon     Control the client daemon
    help       Prints this message or the help of the given subcommand(s)
    list       List information from the library
    load       Load a playlist as the current playlist
    next       Play the next song in the current playlist
    pause      Suspend playback of the current playlist
    play       Play the current playlist
    prev       Play the previous song in the current playlist
    random     Load a number of random songs
    search     Search the library; default returns only songs
    shuffle    Shuffle the curent playlist
    status     Display the status of the daemon
    toggle     Toggle between playing or paused states
    update     Initiate a scan of the library
```

Note that some of the functionality is not yet implemented, and new
functionality will likely be added.

Examples of basic operations:

```sh
$ subs random 50 # adds 50 random songs to the queue
$ subs play

$ subs addnext bad micheal jackson
Adding Bad.
$ subs next

$ subs search micheal jackson # default only searches for songs
Panic! at the Disco feat. Lolo - Too Weird to Live, Too Rare to Die! [2013] - Miss Jackson
Michael Jackson - The Essential Michael Jackson [2005] - Smooth Criminal
The Jackson 5 - The Essential Michael Jackson [2005] - Enjoy Yourself
The Jackson 5 - The Essential Michael Jackson [2005] - Blame It on the Boogie
...

$ subs search -a micheal jackson # artists
Micheal Jackson

$ subs search -b micheal jackson # albums
Michael Jackson - The Essential Michael Jackson [2005]
Michael Jackson - XSCAPE [2014]

$ subs status
Micheal Jackson - Bad
[playing]  #2/51  1:24/4:07 (33%)
```

# License

Licensed under the Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)
or http://www.apache.org/licenses/LICENSE-2.0).

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
licensed as above, without any additional terms or conditions.
