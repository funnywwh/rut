use std::io::*;
use std::net::*;
use std::sync::Arc;
use std::thread;
use std::time;

fn main() -> std::io::Result<()> {
    thread::spawn(move || {
        let svr = Server::listen("localhost:9000").expect("listen failed");
        loop {
            match svr.accept() {
                Ok(StreamType::InStream(recver)) => {
                    thread::spawn(move || loop {
                        let buf = &mut [0; MTU];
                        if let Ok(n) = recver.recv(buf) {
                            println!("instream recv len:{}", n);
                        }
                    });
                }
                Ok(StreamType::OutStream(sender)) => {
                    thread::spawn(move || {
                        let buf = &[0; 100];
                        if let Ok(n) = sender.send(buf, 3) {
                            println!("outstream send len:{}", n);
                        }
                    });
                }
                Ok(StreamType::Known) => {}
                Err(_e) => {
                    eprintln!("err:{}", _e);
                }
            }
        }
    });
    let sender = Sender::connect("localhost:0", "localhost:9000").expect("connect failed");
    loop {
        let buf = &[0; MTU + 120];
        if let Ok(n) = sender.send(buf, 3) {
            println!("Sender send n:{}", n);
        }
        thread::sleep(time::Duration::from_millis(1));
    }
}
enum FrameType {
    KNOWN,
    SENDER,
    RECVER,
}

const RECVED: u8 = 1;
const MTU: usize = 1320;
const SENDER: u8 = 1;
const RECVER: u8 = 2;
const PING: u8 = 3;
const PONG: u8 = 4;
const RTP: u8 = 0x80;

impl FrameType {
    fn new(t: u8) -> FrameType {
        if t == SENDER {
            return FrameType::SENDER;
        } else if t == RECVER {
            return FrameType::RECVER;
        }
        FrameType::KNOWN
    }
}
enum StreamType {
    InStream(Recver),
    OutStream(Sender),
    Known,
}

pub struct Recver {
    udp: Udp,
}
impl Recver {
    pub fn connect<A: ToSocketAddrs>(la: A, ra: A) -> std::io::Result<Recver> {
        let _s = UdpSocket::bind(la)?;

        let ret = Recver {
            udp: Udp { s: Arc::new(_s) },
        };
        let buf = &mut [0; 1];
        buf[0] = RECVER;
        if let Some(_ra) = ra.to_socket_addrs()?.next() {
            ret.udp.connect_to(buf, _ra, 3)?;
        } else {
            return Err(Error::new(ErrorKind::AddrNotAvailable, "remote address"));
        }
        Ok(ret)
    }

    pub fn attach(s: Arc<UdpSocket>) -> std::io::Result<Recver> {
        let ret = Recver {
            udp: Udp::attach(s)?,
        };
        Ok(ret)
    }
    pub fn recv(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.udp.recv(buf)
    }
}
pub struct Sender {
    udp: Udp,
}
impl Sender {
    pub fn connect<A: ToSocketAddrs>(la: A, ra: A) -> std::io::Result<Sender> {
        let _s = UdpSocket::bind(la)?;

        let ret = Sender {
            udp: Udp { s: Arc::new(_s) },
        };
        let buf = &mut [0; 1];
        buf[0] = SENDER;
        if let Some(_ra) = ra.to_socket_addrs()?.next() {
            ret.udp.connect_to(buf, _ra, 3)?;
        } else {
            return Err(Error::new(ErrorKind::AddrNotAvailable, "remote address"));
        }

        Ok(ret)
    }

    pub fn attach(s: Arc<UdpSocket>) -> std::io::Result<Sender> {
        let ret = Sender {
            udp: Udp::attach(s)?,
        };
        Ok(ret)
    }
    pub fn close() -> std::io::Result<u32> {
        Ok(0)
    }
    pub fn send(&self, buf: &[u8], retry: u8) -> std::io::Result<usize> {
        let mut nsent: usize = 0;
        let mut left = buf.len();
        loop {
            if left <= 0 {
                break;
            }
            let b = &buf[nsent..nsent + if left > MTU { MTU } else { left }];
            let n = self.udp.retry_send(b, retry)?;
            nsent += n;
            left -= n;
        }
        Ok(nsent)
    }
    pub fn set_retry_timeout(&self, dur: Option<time::Duration>) -> std::io::Result<()> {
        self.udp.set_read_timeout(dur)
    }
}

struct Server {
    udp: Udp,
    bindAddr: SocketAddr,
}

impl Server {
    pub fn listen<A: ToSocketAddrs>(la: A) -> std::io::Result<Server> {
        if let Some(local_socket_addr) = la.to_socket_addrs()?.next() {
            let socket = UdpSocket::bind(la)?;

            let arc_socket = Arc::new(socket);
            let ret = Server {
                udp: Udp { s: arc_socket },
                bindAddr: local_socket_addr,
            };
            Ok(ret)
        } else {
            return Err(Error::new(
                ErrorKind::AddrNotAvailable,
                "listen address not available",
            ));
        }
    }
    pub fn accept(&self) -> Result<StreamType> {
        let ft = &mut [0; 1];
        let (_, src) = self.udp.recv_from(ft)?;
        let mut new_addr = self.bindAddr.clone();
        new_addr.set_port(0);
        let arc_socket = Arc::new(UdpSocket::bind(new_addr)?);

        match FrameType::new(ft[0]) {
            FrameType::SENDER => {
                println!("accept sender");
                let ret = StreamType::InStream(Recver::attach(arc_socket.clone())?);
                arc_socket.send_to(&[RECVED], src)?;
                return Ok(ret);
            }
            FrameType::RECVER => {
                let ret = StreamType::OutStream(Sender::attach(arc_socket.clone())?);
                arc_socket.send_to(&[RECVED], src)?;
                return Ok(ret);
            }
            FrameType::KNOWN => {}
        }
        Ok(StreamType::Known)
    }
}
struct Udp {
    s: Arc<UdpSocket>,
}
impl Udp {
    pub fn connect_to(
        &self,
        buf: &[u8],
        ra: SocketAddr,
        mut retry: u8,
    ) -> Result<(usize, SocketAddr)> {
        let r = retry;
        let mut ack = [0; 1];

        loop {
            if retry == 0 {
                break;
            }

            if let Ok(_) = self.s.send_to(buf, ra.clone()) {
                self.set_read_timeout(Some(time::Duration::from_secs(1)))?;
                if let Ok((_, src)) = self.s.recv_from(&mut ack) {
                    if ack[0] != RECVED {
                        return Err(Error::new(
                            ErrorKind::Other,
                            format!("ack {} != {}", ack[0], RECVED),
                        ));
                    }
                    self.s.connect(src)?;
                    return Ok((buf.len(), src));
                }
            }
            retry -= 1;
        }
        Err(std::io::Error::new(
            ErrorKind::Other,
            format!("send failed after retry {}.", r),
        ))
    }
    fn retry_send(&self, buf: &[u8], mut retry: u8) -> std::io::Result<usize> {
        let r = retry;
        let mut ack = [0; 1];
        loop {
            if retry == 0 {
                break;
            }

            if let Ok(_) = self.s.send(buf) {
                if let Ok(_) = self.s.recv(&mut ack) {
                    if ack[0] != 1 {
                        return Err(Error::new(ErrorKind::Other, format!("ack {} != 1", ack[0])));
                    }
                    return Ok(buf.len());
                }
            }

            retry -= 1;
        }
        Err(std::io::Error::new(
            ErrorKind::Other,
            format!("send failed after retry {}.", r),
        ))
    }
    pub fn send(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.s.send(buf)
    }
    pub fn recv(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        let (n, src) = self.s.recv_from(buf)?;
        let b = &[1];
        self.s.send_to(b, src)?;
        Ok(n)
    }
    pub fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        self.s.recv_from(buf)
    }
    pub fn attach(s: Arc<UdpSocket>) -> std::io::Result<Udp> {
        let ret = Udp { s: s };
        Ok(ret)
    }
    pub fn set_read_timeout(&self, dur: Option<time::Duration>) -> std::io::Result<()> {
        self.s.set_read_timeout(dur)
    }
}
