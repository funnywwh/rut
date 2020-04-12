use std::net::*;
use std::sync::Arc;
use std::io::*;
use std::thread;
use std::time;

fn main() -> std::io::Result<()> {
    thread::spawn(move || {
        let svr = Server::listen("localhost:9000").expect("listen failed");
        loop{
            match svr.accept(){
                Ok(StreamType::In(_s)) =>{

                }
                Ok(StreamType::Out(_s)) =>{

                }
                Ok(StreamType::Known) =>{

                }
                Err(_e) =>{
                    eprintln!("err:{}",_e);
                }
            }
        }
    });
    let sender = Sender::connect("localhost:9001", "localhost:9000").expect("connect failed");
    loop{
        let buf = &[0;MTU+120];
        if let Ok(n) = sender.send(buf, 3){
            println!("Sender send n:{}",n);
        }
        thread::sleep(time::Duration::from_millis(1));
    }

} 

const MTU:usize = 1320;
const SENDER:u8 = 1;
const RECVER:u8 = 2;
const DATA:u8 = 3;//<frametype><data length><data>
const RTP:u8 = 0x80;
enum FrameType{
    KNOWN,
    SENDER,
    RECVER,
    DATA,
    RTP,
}
impl FrameType{
    fn new(t :u8)->FrameType{
        if t == SENDER {
            return FrameType::SENDER;
        }
        FrameType::KNOWN
    }
}

struct Sender{
    s : Arc<UdpSocket>
}
impl Sender{
    
    pub fn connect<A: ToSocketAddrs>(la:A,ra :A) -> std::io::Result<Sender>{
        let _s = UdpSocket::bind(la)?;
        
        let ret = Sender{s:Arc::new(_s)};
        let buf = &mut [0;1];
        buf[0] = SENDER;
        ret.s.connect(ra)?;
        ret.retry_send(buf, 3)?;
        Ok(ret)
    }
    pub fn attach(s:Arc<UdpSocket>)->std::io::Result<Sender>{
        let ret = Sender{s:s};
        Ok(ret)
    }
    pub fn close()->std::io::Result<u32>{
        Ok(0)
    }
    pub fn send(&self,buf :&[u8],retry:u8)->std::io::Result<usize>{
        let mut nsent:usize = 0;
        let mut left = buf.len();
        loop{
            if left <=0 {
                break;
            }
            let b = &buf[nsent.. nsent + if left > MTU { MTU }else{left} ];
            let n = self.retry_send(b,retry)?;
            nsent += n;
            left -= n;
        }
        Ok(nsent)
    }
    pub fn set_retry_timeout(&self,dur :Option<time::Duration>)->std::io::Result<()>{
        self.s.set_read_timeout(dur)
    }
    fn retry_send(&self,buf:&[u8],mut retry:u8)->std::io::Result<usize>{
        let r = retry;
        let mut ack =[0;1];
        loop{
            if retry == 0 {
                break;
            }
            
            if let Ok(_) = self.s.send(buf){
                if let Ok(_) = self.s.recv(&mut ack){
                    if ack[0] != 1{
                        return Err(Error::new(ErrorKind::Other,format!("ack {} != 1",ack[0])));
                    }
                    return Ok(buf.len());
                }
            }
            
            retry -= 1;
        }
        Err(std::io::Error::new(ErrorKind::Other, format!("send failed after retry {}.",r)))
    }
}
struct Recver{
    s : Arc<UdpSocket>
}
impl Recver{
    pub fn connect<A: ToSocketAddrs>(la:A,ra :A) -> std::io::Result<Sender>{
        let _s = UdpSocket::bind(la)?;
        
        let ret = Sender{s:Arc::new(_s)};
        let buf = &mut [0;1];
        buf[0] = RECVER;
        ret.s.connect(ra)?;
        ret.retry_send(buf, 3)?;
        Ok(ret)
    }   
    pub fn attach(s:Arc<UdpSocket>)->std::io::Result<Recver>{
        let ret = Recver{s:s};
        Ok(ret)
    }    
    pub fn recv(&self,buf:&mut [u8])->std::io::Result<usize>{
        self.s.set_read_timeout(None)?;
        let (n,src) = self.s.recv_from(buf)?;
        let b = &[1];
        self.s.send_to(b, src)?;
        Ok(n)
    }
}

struct Server{
    sender:Sender,
    recver:Recver,
}


impl Server{
    pub fn listen<A:ToSocketAddrs>(la:A)->std::io::Result<Server>{
        let socket = UdpSocket::bind(la)?;
        let arc = Arc::new(socket);
        let ret = Server{
            sender:Sender::attach(arc.clone())?,
            recver:Recver::attach(arc.clone())?,
        };
        Ok(ret)
    }
    pub fn accept(&self)->Result<StreamType>{
        let ft = &mut [0];
        let nr = self.recver.recv(ft)?;
        match FrameType::new(ft[0]) {
            FrameType::SENDER =>{

            }
            FrameType::RECVER =>{

            }
            FrameType::DATA =>{

            }
            FrameType::RTP =>{

            }
            FrameType::KNOWN =>{

            }
        }
        Ok(StreamType::Known)
    }
}

enum StreamType{
    In(InStream),
    Out(OutStream),
    Known,
}
struct InStream{

}
struct OutStream{

}


