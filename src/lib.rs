use std::collections::{VecDeque, HashMap, hash_map};
use std::error::Error;
use std::io::{Read, Write};
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex, Condvar};
use std::{thread, io};


use tun_tap;
use smoltcp::wire::{self, PrettyPrinter};

mod tcp;
const SENDQUEUE_SIZE: usize = 1024;



type Port = u16;
type Connection = tcp::Connection;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
struct Quad {
    src: (Ipv4Addr, Port),
    dst: (Ipv4Addr, Port),
}

struct ConnectionPool_inner {
    connection: Mutex<HashMap<Quad, Arc<(Mutex<Connection>,Condvar)>>>,
    listener: Mutex<HashMap<Port, VecDeque<Quad>>>,
    pending_var: Condvar,
}

impl Default for ConnectionPool_inner {
    fn default() -> Self {
        ConnectionPool_inner::new()
    }
}

impl ConnectionPool_inner {
    fn new() -> Self {
        Self {
            connection: Mutex::default(),
            listener: Mutex::default(),
            pending_var: Condvar::default()
        }
    }
    fn add_connection(&self, quad:Quad, cn:Connection) -> Arc<(Mutex<Connection>, Condvar)>{
        let cn = Arc::new((Mutex::new(cn), Condvar::default()));
        self.connection.lock().unwrap().insert(quad, cn.clone());
        cn
    }
    fn get(&self, q: &Quad) -> Option<Arc<(Mutex<Connection>, Condvar)>> {
        self.connection.lock().unwrap().get(q).map(|c| {
            c.clone()
        })
    }
    fn remove(&self, quad:Quad) -> Option<Arc<(Mutex<Connection>, Condvar)>> {
        self.connection.lock().unwrap().remove(&quad)
    }
    fn bind(&self, port: Port) -> io::Result<()>{
        if self.listener.lock().unwrap().contains_key(&port) {
            Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "port already bound",
            ))
        } else {
            self.listener.lock().unwrap().insert(port, VecDeque::default());
            Ok(())
        }
    }
}

type ConnectionPool = Arc<ConnectionPool_inner>;

pub struct Interface {
    connection_pool: ConnectionPool,
}

// Establish a connection Remove a connection Send messge to connection.
fn packet_loop( mut nic:tun_tap::Iface, connection_pool: ConnectionPool) -> io::Result<()>{
    let mut buf = [0u8; 1504];
    let mut try_recv = move || -> Result<(), Box<dyn Error>> {
        use std::os::unix::io::AsRawFd;
        let mut pfd = [nix::poll::PollFd::new(
            nic.as_raw_fd(),
            nix::poll::EventFlags::POLLIN,
            )];
        let n = nix::poll::poll(&mut pfd[..], 10).map_err(|e| e.as_errno().unwrap())?;
        assert_ne!(n, -1);
        if n == 0 {
            let mut cmg = connection_pool.connection.lock().unwrap();
            for connection in cmg.values_mut() {
                // XXX: don't die on errors?
                connection.0.lock().unwrap().on_tick(&mut nic)?;
            }
            return Err(Box::new(io::Error::new(io::ErrorKind::AddrInUse, "n is not 1",)));
        }
        assert_eq!(n, 1);
        let nbytes = nic.recv(&mut buf[..])?;
        println!(
            "{}",
            PrettyPrinter::<wire::Ipv4Packet<&[u8]>>::new("", &&buf[..nbytes])
            );
        let iph = etherparse::Ipv4HeaderSlice::from_slice(&buf[..nbytes])?;
        let tcph = etherparse::TcpHeaderSlice::from_slice(&buf[iph.slice().len()..nbytes])?;
        let q = Quad{
            src: (iph.source_addr(), tcph.source_port()),
            dst: (iph.destination_addr(), tcph.destination_port())
        };
        let cn = &*connection_pool;
        let datai = iph.slice().len() + tcph.slice().len();
        match cn.connection.lock().unwrap().entry(q) {
            hash_map::Entry::Occupied(mut c) => {
                println!("recv message");
                let (cn, var) = &**(c.get_mut());
                let a = cn.lock().unwrap().on_packet(&mut nic, iph, tcph, &buf[datai..nbytes])?;
                var.notify_all();
            },
            hash_map::Entry::Vacant(e) => {
                println!("accept");
                if let Some(listener) = cn.listener.lock().unwrap().get_mut(&tcph.destination_port()) {
                    if let Some(c) = tcp::Connection::accept( &mut nic, iph, tcph, &buf[datai..nbytes],)? {
                        let c = Arc::new((Mutex::new(c), Condvar::default()));
                        e.insert(c);
                        listener.push_back(q);
                        cn.pending_var.notify_all();
                    }
                }
            }
        }
        Ok(())
    };
    loop {
        if let Err(e) = try_recv() {
            // println!("Skip because of {}", &e);
        }
    }
    Ok(())
}

impl Interface {
    pub fn new() -> io::Result<Self> {
        let nic = tun_tap::Iface::without_packet_info("tun0", tun_tap::Mode::Tun)?;
        let con_pool: ConnectionPool = Arc::default();
        let temp = con_pool.clone();
        thread::spawn(move || packet_loop( nic, temp));
        Ok( Self {
            connection_pool: con_pool,
        })
    }
    pub fn bind(&mut self, port: Port) -> io::Result<TcpListener> {
        self.connection_pool.bind(port).map(|_| {
            TcpListener{
                port,
                con_pool: self.connection_pool.clone()
            }
        })
    }
    pub fn connect() -> io::Result<TcpStream> {
        unimplemented!()
    }
}

pub struct TcpListener {
    port: Port,
    con_pool: ConnectionPool
}

impl TcpListener {
    pub fn accept(&mut self) -> io::Result<TcpStream> {
        loop {
            let mut listener = self.con_pool.listener.lock().unwrap();
            if let Some(quad) = listener.get_mut(&self.port).unwrap().pop_front() {
                return Ok(TcpStream{
                    quad,
                    connection: self.con_pool.connection.lock().unwrap().get_mut(&quad).unwrap().clone()
                })
            }
            listener = self.con_pool.pending_var.wait(listener).unwrap();
        }
    }
}

pub struct TcpStream {
    quad: Quad,
    connection: Arc<(Mutex<Connection>, Condvar)>
}


impl Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let (c, var) = &*self.connection;
            let mut c = c.lock().unwrap();

            if c.is_rcv_closed() && c.incoming.is_empty() {
                // no more data to read, and no need to block, because there won't be any more
                return Ok(0);
            }

            if !c.incoming.is_empty() {
                let mut nread = 0;
                let (head, tail) = c.incoming.as_slices();
                let hread = std::cmp::min(buf.len(), head.len());
                buf[..hread].copy_from_slice(&head[..hread]);
                nread += hread;
                let tread = std::cmp::min(buf.len() - nread, tail.len());
                buf[hread..(hread + tread)].copy_from_slice(&tail[..tread]);
                nread += tread;
                drop(c.incoming.drain(..nread));
                return Ok(nread);
            }
            c = var.wait(c).unwrap();
        }
    }
}

impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let c = &mut self.connection.0.lock().unwrap();

        if c.unacked.len() >= SENDQUEUE_SIZE {
            // TODO: block
            return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "too many bytes buffered",
                    ));
        }

        let nwrite = std::cmp::min(buf.len(), SENDQUEUE_SIZE - c.unacked.len());
        c.unacked.extend(buf[..nwrite].iter());

        Ok(nwrite)
    }

    fn flush(&mut self) -> io::Result<()> {
        let c = &mut self.connection.0.lock().unwrap();

        if c.unacked.is_empty() {
            Ok(())
        } else {
            // TODO: block
            Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "too many bytes buffered",
                    ))
        }
    }
}

impl TcpStream {
    pub fn shutdown(&self, how: std::net::Shutdown) -> io::Result<()> {
        let c = &mut self.connection.0.lock().unwrap();
        c.close()
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::*;
    // #[test]
    fn test_packet_loop() {
        let nic = tun_tap::Iface::without_packet_info("tap0", tun_tap::Mode::Tap).expect("Failed Creat nic");
        let con_pool: ConnectionPool = Arc::default();
        packet_loop(nic, con_pool);
    } 
}
