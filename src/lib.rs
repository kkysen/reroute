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
    cookie: u32,
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
            cookie: 0,
        })
    }
    
    fn run(&mut self) -> io::Result<()> {
        if !self.from.is_dir() || !self.to.is_dir() {
            return Err(io::ErrorKind::InvalidInput.into())
        }
        self.inotify.add_watch(self.from, WatchMask::CREATE)?;
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
        let mask = event.mask;
        if mask.contains(EventMask::ISDIR) {
            return Ok(());
        }
        if mask.contains(EventMask::CREATE) {
            if !mask.contains(EventMask::ISDIR) {
                self.re_route_event(event)?;
            }
        } else if mask.contains(EventMask::MOVED_FROM) {
            self.cookie = event.cookie;
        } else if mask.contains(EventMask::MOVED_TO) {
            if self.cookie != event.cookie {
                self.re_route_event(event)?;
            }
            self.cookie = 0;
        }
        Ok(())
    }
    
    fn re_route_event(&self, event: Event<&OsStr>) -> io::Result<()> {
        let event = Event {
            wd: event.wd,
            mask: event.mask,
            cookie: event.cookie,
            name: event.name.map(|it| Path::new(it)),
        };
        if !(self.filter)(&event) {
            return Ok(())
        }
        let from = event.name.unwrap();
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
