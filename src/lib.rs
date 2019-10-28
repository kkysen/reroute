use std::path::{PathBuf, Path};
use std::{env, fs, io};
use inotify::{Inotify, WatchMask, EventMask, Event};
use std::ffi::OsStr;
use std::thread::sleep;
use std::time::Duration;

#[derive(Debug)]
pub struct ReRoute {
    pub from: PathBuf,
    pub to: PathBuf,
}

impl ReRoute {
    pub fn new(from: PathBuf, to: PathBuf) -> ReRoute {
        ReRoute {from, to}
    }
    
    pub fn default(from: Option<PathBuf>, to: Option<PathBuf>) -> ReRoute {
        ReRoute {
            from: from.unwrap_or_else(||
                env::var("DOWNLOADS")
                    .map(|it| PathBuf::from(it))
                    .unwrap()
            ),
            to: to.unwrap_or_else(||
                env::current_dir()
                    .unwrap()
            ),
        }
    }
    
    pub fn run<F, G>(&self, filter: F, on_error: G) -> io::Result<()>
        where F: Fn(&Event<&Path>) -> bool, G: Fn(io::Error) {
        RunningReRouter::new(&self, filter, on_error)?.run()
    }
}

struct RunningReRouter<'a, F, G>
    where F: Fn(&Event<&Path>) -> bool, G: Fn(io::Error) {
    from: &'a Path,
    to: &'a Path,
    filter: F,
    on_error: G,
    inotify: Inotify,
//    cookie: u32, // cookie doesn't work on WSL, use prev instead
    prev: Option<PathBuf>,
    moving: bool,
}

impl<'a, F, G> RunningReRouter<'a, F, G>
    where F: Fn(&Event<&Path>) -> bool, G: Fn(io::Error) {
    fn new(re_route: &'a ReRoute, filter: F, on_error: G) -> io::Result<RunningReRouter<'a, F, G>>
        where G: Fn(io::Error) {
        Ok(RunningReRouter {
            from: re_route.from.as_path(),
            to: re_route.to.as_path(),
            filter,
            on_error,
            inotify: Inotify::init()?,
//            cookie: 0,
            prev: None,
            moving: false,
        })
    }
    
    fn prev(&self) -> Option<&Path> {
        self.prev.as_ref().map(|it| it.as_path())
    }
    
    fn run(&mut self) -> io::Result<()> {
        if !self.from.is_dir() || !self.to.is_dir() {
            return Err(io::ErrorKind::InvalidInput.into())
        }
        let mask = WatchMask::CREATE | WatchMask::MOVE | WatchMask::ONLYDIR;
        self.inotify.add_watch(self.from, mask)?;
        let mut buffer = [0u8; 4096];
        loop {
            let events = self.inotify.read_events_blocking(buffer.as_mut())?;
            for event in events {
                if let Err(e) = self.handle_event(event) {
                    (self.on_error)(e);
                }
            }
        }
    }
    
    fn handle_event(&mut self, event: Event<&OsStr>) -> io::Result<()> {
        dbg!(&event);
        let mask = event.mask;
        if mask.contains(EventMask::ISDIR) {
            return Ok(());
        }
        let event = Event {
            wd: event.wd,
            mask: event.mask,
            cookie: event.cookie,
            name: event.name.map(|it| Path::new(it)),
        };
        if mask.contains(EventMask::CREATE) {
            if !mask.contains(EventMask::ISDIR) {
                self.re_route_event(&event)?;
            }
        } else if mask.contains(EventMask::MOVED_FROM) {
//            self.cookie = event.cookie;
            self.prev = event.name.map(|it| it.into());
            self.moving = true;
        } else if mask.contains(EventMask::MOVED_TO) {
            dbg!(&self.cookie);
            dbg!(&self.prev);
            dbg!(&event);
            let same_as_prev = self.prev() == event.name;
            if (moving && !same_as_prev) || same_as_prev {
            
            }
            if /*self.cookie != event.cookie ||*/ self.prev() == event.name {
                self.re_route_event(&event)?;
            }
//            self.cookie = 0;
            self.moving = false;
        }
        Ok(())
    }
    
    fn re_route_event(&mut self, event: &Event<&Path>) -> io::Result<()> {
        let from = event.name.unwrap();
        if !(self.filter)(event) {
            self.prev = Some(from.into());
            self.moving = false;
            return Ok(())
        }
        self.prev = None;
        let to = from;
        let from = self.from.join(from);
        let to = self.to.join(to);
        if to.exists() {
            Err(io::ErrorKind::AlreadyExists.into())
        } else {
            println!("{:?} => {:?}", from, to);
            if let Err(e) = fs::rename(&from, &to) {
                if e.kind() == io::ErrorKind::NotFound {
                    // need to sleep b/c sometimes the file isn't actually there yet
                    sleep(Duration::from_millis(1000));
                    fs::rename(&from, &to)
                } else {
                    Err(e)
                }
            } else {
                Ok(())
            }
        }
    }
}
